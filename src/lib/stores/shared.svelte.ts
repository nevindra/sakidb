import { decode } from '@msgpack/msgpack';
import type { ColumnDef, ColumnArray } from '$lib/types';
import { ColumnarResultData } from '$lib/types/query-result-data';

const DEBUG = import.meta.env.DEV;

// ── Shared error state ──

let error = $state<string | null>(null);

export function getError(): string | null { return error; }
export function setError(msg: string) { error = msg; }
export function clearError() { error = null; }

// ── Utilities ──

export function decodeMsgpack<T>(bytes: ArrayBuffer | number[]): T {
  const buf = bytes instanceof ArrayBuffer ? new Uint8Array(bytes) : new Uint8Array(bytes);
  return decode(buf) as T;
}

/**
 * Decode a multi-result columnar binary buffer from Rust.
 *
 * Multi-result framing: u32 result_count, u64 total_execution_time_ms,
 * then for each result: u32 byte_length, <encoded ColumnarResult bytes>
 */
export function decodeMultiColumnar(buffer: ArrayBuffer): {
  results: ColumnarResultData[];
  total_execution_time_ms: number;
} {
  const t0 = performance.now();
  const view = new DataView(buffer);
  const resultCount = view.getUint32(0, true);
  const totalExecTime = Number(view.getBigUint64(4, true));
  let offset = 12;

  if (DEBUG) console.log(`[columnar] IPC buffer: ${(buffer.byteLength / 1024 / 1024).toFixed(1)} MB, ${resultCount} result(s)`);

  const results: ColumnarResultData[] = [];
  for (let i = 0; i < resultCount; i++) {
    const byteLen = view.getUint32(offset, true);
    offset += 4;
    // Pass buffer + offset directly — avoids copying the entire result payload
    results.push(decodeSingleColumnar(buffer, offset));
    offset += byteLen;
  }

  if (DEBUG) console.log(`[columnar] decode took ${(performance.now() - t0).toFixed(1)} ms`);
  return { results, total_execution_time_ms: totalExecTime };
}

function decodeSingleColumnar(buffer: ArrayBuffer, baseOffset: number): ColumnarResultData {
  const view = new DataView(buffer);
  const bytes = new Uint8Array(buffer);
  let offset = baseOffset;

  // Header (25 bytes)
  const colCount = view.getUint32(offset, true); offset += 4;
  const rowCount = Number(view.getBigUint64(offset, true)); offset += 8;
  const execTimeMs = Number(view.getBigUint64(offset, true)); offset += 8;
  const truncated = bytes[offset] !== 0; offset += 1;
  offset += 4; // padding

  // Column definitions
  const columns: ColumnDef[] = [];
  const decoder = new TextDecoder();
  for (let i = 0; i < colCount; i++) {
    const nameLen = view.getUint16(offset, true); offset += 2;
    const name = decoder.decode(bytes.subarray(offset, offset + nameLen)); offset += nameLen;
    const typeLen = view.getUint16(offset, true); offset += 2;
    const dataType = decoder.decode(bytes.subarray(offset, offset + typeLen)); offset += typeLen;
    columns.push({ name, data_type: dataType });
  }

  // Column data
  const columnData: ColumnArray[] = [];
  let totalExtracted = 0;
  for (let i = 0; i < colCount; i++) {
    const typeTag = bytes[offset]; offset += 1;
    // COPY nulls — a view would pin the entire IPC buffer in memory
    const nulls = bytes.slice(offset, offset + rowCount); offset += rowCount;
    totalExtracted += rowCount; // nulls

    switch (typeTag) {
      case 0: { // Number
        // Align to 8 bytes for Float64Array
        const padding = (8 - (offset % 8)) % 8;
        offset += padding;
        const values = new Float64Array(buffer.slice(offset, offset + rowCount * 8));
        offset += rowCount * 8;
        totalExtracted += rowCount * 8;
        columnData.push({ type: 'number', nulls, values });
        if (DEBUG) console.log(`[columnar]   col[${i}] "${columns[i].name}" (${columns[i].data_type}): number, ${(rowCount * 8 / 1024 / 1024).toFixed(2)} MB`);
        break;
      }
      case 1: { // Bool
        // COPY — a view would pin the entire IPC buffer
        const values = bytes.slice(offset, offset + rowCount);
        offset += rowCount;
        totalExtracted += rowCount;
        columnData.push({ type: 'bool', nulls, values });
        if (DEBUG) console.log(`[columnar]   col[${i}] "${columns[i].name}" (${columns[i].data_type}): bool, ${(rowCount / 1024).toFixed(1)} KB`);
        break;
      }
      case 2: { // Text
        const dataLen = view.getUint32(offset, true); offset += 4;
        // COPY text blob — allows the IPC ArrayBuffer to be GC'd once decode completes.
        // A subarray view would pin the entire buffer (including already-copied number/bool data).
        const rawData = bytes.slice(offset, offset + dataLen); offset += dataLen;
        // Bulk copy offsets — faster than rowCount individual getUint32 calls
        const rawOffsets = new Uint32Array(buffer.slice(offset, offset + (rowCount + 1) * 4));
        offset += (rowCount + 1) * 4;
        totalExtracted += (rowCount + 1) * 4;
        // Sparse values array — populated lazily by ColumnarResultData._getText()
        // Empty array avoids V8 pre-allocating pointer slots for rowCount entries.
        const values: (string | undefined)[] = [];
        columnData.push({ type: 'text', nulls, values, _raw: rawData, _offsets: rawOffsets });
        if (DEBUG) console.log(`[columnar]   col[${i}] "${columns[i].name}" (${columns[i].data_type}): text, raw=${(dataLen / 1024 / 1024).toFixed(2)} MB, offsets=${((rowCount + 1) * 4 / 1024 / 1024).toFixed(2)} MB`);
        break;
      }
      case 3: { // Bytes
        const dataLen = view.getUint32(offset, true); offset += 4;
        const data = bytes.subarray(offset, offset + dataLen); offset += dataLen;
        const offsets = new Uint32Array(buffer.slice(offset, offset + (rowCount + 1) * 4));
        offset += (rowCount + 1) * 4;

        let bytesCopied = 0;
        const values: Uint8Array[] = new Array(rowCount);
        for (let r = 0; r < rowCount; r++) {
          if (nulls[r] !== 0) {
            values[r] = new Uint8Array(0);
          } else {
            values[r] = new Uint8Array(
              buffer.slice(
                data.byteOffset + offsets[r],
                data.byteOffset + offsets[r + 1],
              ),
            );
            bytesCopied += values[r].byteLength;
          }
        }
        totalExtracted += bytesCopied + (rowCount + 1) * 4;
        columnData.push({ type: 'bytes', nulls, values });
        if (DEBUG) console.log(`[columnar]   col[${i}] "${columns[i].name}" (${columns[i].data_type}): bytes, data=${(bytesCopied / 1024 / 1024).toFixed(2)} MB (${rowCount} rows)`);
        break;
      }
    }
  }

  if (DEBUG) console.log(`[columnar] ${rowCount} rows, ${colCount} cols, extracted=${(totalExtracted / 1024 / 1024).toFixed(1)} MB`);
  return new ColumnarResultData(columns, columnData, rowCount, execTimeMs, truncated);
}

export function generateId(): string {
  return crypto.randomUUID();
}

export function isCancelError(msg: string): boolean {
  return msg.includes('canceling statement due to user request');
}
