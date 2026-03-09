import type { AnyQueryResult, QueryResult } from '$lib/types';
import { ColumnarResultData } from '$lib/types/query-result-data';

// ── Cell value access (works for both QueryResult and ColumnarResultData) ──

function getValueFromResult(
  result: AnyQueryResult,
  row: number,
  col: number,
): string {
  if (result instanceof ColumnarResultData) {
    if (result.isNull(row, col)) return '\0NULL';
    const v = result.getValue(row, col);
    if (v instanceof Uint8Array) {
      return Array.from(v.slice(0, 32))
        .map((b) => b.toString(16).padStart(2, '0'))
        .join(' ');
    }
    return String(v);
  }
  // QueryResult (row-based)
  const qr = result as QueryResult;
  const cell = qr.cells[row * qr.columns.length + col];
  if (cell === 'Null') return '\0NULL';
  if ('Bool' in cell) return String(cell.Bool);
  if ('Int' in cell) return String(cell.Int);
  if ('Float' in cell) return String(cell.Float);
  if ('Text' in cell) return cell.Text;
  if ('Json' in cell) return cell.Json;
  if ('Timestamp' in cell) return cell.Timestamp;
  if ('Bytes' in cell)
    return cell.Bytes.map((b) => b.toString(16).padStart(2, '0')).join(' ');
  return '';
}

// ── Diff types ──

export interface DiffMap {
  /** Set of "row:col" keys for cells that differ */
  changed: Set<string>;
  /** Row indices in result A that have no match in B (removed) */
  removedRows: Set<number>;
  /** Row indices in result B that have no match in A (added) */
  addedRows: Set<number>;
  /** Summary stats */
  changedCellCount: number;
  columnsMatch: boolean;
}

// ── Position-based diff ──

export function diffByPosition(
  a: AnyQueryResult,
  b: AnyQueryResult,
): DiffMap {
  const changed = new Set<string>();
  const removedRows = new Set<number>();
  const addedRows = new Set<number>();

  const colsA = a.columns.length;
  const colsB = b.columns.length;
  const minCols = Math.min(colsA, colsB);
  const minRows = Math.min(a.row_count, b.row_count);

  // Compare overlapping cells
  for (let row = 0; row < minRows; row++) {
    for (let col = 0; col < minCols; col++) {
      const va = getValueFromResult(a, row, col);
      const vb = getValueFromResult(b, row, col);
      if (va !== vb) {
        changed.add(`${row}:${col}`);
      }
    }
  }

  // Extra rows in A → removed
  for (let row = minRows; row < a.row_count; row++) {
    removedRows.add(row);
  }
  // Extra rows in B → added
  for (let row = minRows; row < b.row_count; row++) {
    addedRows.add(row);
  }

  const columnsMatch =
    colsA === colsB &&
    a.columns.every((c, i) => c.name === b.columns[i].name);

  return {
    changed,
    removedRows,
    addedRows,
    changedCellCount: changed.size,
    columnsMatch,
  };
}

// ── Key-based diff ──

export function diffByKey(
  a: AnyQueryResult,
  b: AnyQueryResult,
  keyColumn: string,
): DiffMap {
  const changed = new Set<string>();
  const removedRows = new Set<number>();
  const addedRows = new Set<number>();

  const keyColA = a.columns.findIndex((c) => c.name === keyColumn);
  const keyColB = b.columns.findIndex((c) => c.name === keyColumn);

  // If key column not found in either result, fall back to position
  if (keyColA === -1 || keyColB === -1) {
    return diffByPosition(a, b);
  }

  // Build key→rowIndex map for B
  const bMap = new Map<string, number>();
  for (let row = 0; row < b.row_count; row++) {
    const key = getValueFromResult(b, row, keyColB);
    bMap.set(key, row);
  }

  const matchedBRows = new Set<number>();

  // Find common columns (by name)
  const colPairs: [number, number][] = [];
  for (let ca = 0; ca < a.columns.length; ca++) {
    const cb = b.columns.findIndex((c) => c.name === a.columns[ca].name);
    if (cb !== -1) colPairs.push([ca, cb]);
  }

  // Compare rows
  for (let rowA = 0; rowA < a.row_count; rowA++) {
    const key = getValueFromResult(a, rowA, keyColA);
    const rowB = bMap.get(key);

    if (rowB === undefined) {
      removedRows.add(rowA);
      continue;
    }

    matchedBRows.add(rowB);

    // Compare matching columns
    for (const [ca, cb] of colPairs) {
      const va = getValueFromResult(a, rowA, ca);
      const vb = getValueFromResult(b, rowB, cb);
      if (va !== vb) {
        changed.add(`${rowA}:${ca}`);
      }
    }
  }

  // Rows in B with no match in A → added
  for (let row = 0; row < b.row_count; row++) {
    if (!matchedBRows.has(row)) {
      addedRows.add(row);
    }
  }

  const columnsMatch =
    a.columns.length === b.columns.length &&
    a.columns.every((c, i) => c.name === b.columns[i].name);

  return {
    changed,
    removedRows,
    addedRows,
    changedCellCount: changed.size,
    columnsMatch,
  };
}

// ── Export utilities ──

function escapeField(value: string, delimiter: string): string {
  if (
    value.includes(delimiter) ||
    value.includes('"') ||
    value.includes('\n') ||
    value.includes('\r')
  ) {
    return `"${value.replace(/"/g, '""')}"`;
  }
  return value;
}

/**
 * Convert a query result to delimited text (CSV or TSV).
 * Uses getValue() fast path for columnar data — no CellValue construction.
 */
export function resultToDelimited(
  result: AnyQueryResult,
  delimiter: string,
): string {
  const colCount = result.columns.length;
  const rowCount = result.row_count;
  const lines: string[] = [];

  // Header
  lines.push(
    result.columns.map((c) => escapeField(c.name, delimiter)).join(delimiter),
  );

  // Rows
  for (let row = 0; row < rowCount; row++) {
    const fields: string[] = [];
    for (let col = 0; col < colCount; col++) {
      const val = getValueFromResult(result, row, col);
      fields.push(
        val === '\0NULL' ? '' : escapeField(val, delimiter),
      );
    }
    lines.push(fields.join(delimiter));
  }

  return lines.join('\n');
}

/**
 * Convert a query result to a JSON array of objects.
 * Uses getValue() fast path for columnar data.
 */
export function resultToJson(result: AnyQueryResult): string {
  const colCount = result.columns.length;
  const rowCount = result.row_count;
  const colNames = result.columns.map((c) => c.name);
  const rows: Record<string, unknown>[] = [];

  for (let row = 0; row < rowCount; row++) {
    const obj: Record<string, unknown> = {};
    for (let col = 0; col < colCount; col++) {
      if (result instanceof ColumnarResultData) {
        if (result.isNull(row, col)) {
          obj[colNames[col]] = null;
          continue;
        }
        const cd = result.columnData[col];
        if (cd.type === 'number') {
          obj[colNames[col]] = cd.values[row];
        } else if (cd.type === 'bool') {
          obj[colNames[col]] = cd.values[row] !== 0;
        } else {
          const v = result.getValue(row, col);
          // Try to parse JSON columns as objects
          const dt = result.columns[col].data_type.toLowerCase();
          if ((dt === 'json' || dt === 'jsonb') && typeof v === 'string') {
            try {
              obj[colNames[col]] = JSON.parse(v);
            } catch {
              obj[colNames[col]] = v;
            }
          } else {
            obj[colNames[col]] = v instanceof Uint8Array ? Array.from(v) : v;
          }
        }
      } else {
        // QueryResult
        const qr = result as QueryResult;
        const cell = qr.cells[row * colCount + col];
        if (cell === 'Null') {
          obj[colNames[col]] = null;
        } else if ('Bool' in cell) {
          obj[colNames[col]] = cell.Bool;
        } else if ('Int' in cell) {
          obj[colNames[col]] = cell.Int;
        } else if ('Float' in cell) {
          obj[colNames[col]] = cell.Float;
        } else if ('Text' in cell) {
          obj[colNames[col]] = cell.Text;
        } else if ('Json' in cell) {
          try {
            obj[colNames[col]] = JSON.parse(cell.Json);
          } catch {
            obj[colNames[col]] = cell.Json;
          }
        } else if ('Timestamp' in cell) {
          obj[colNames[col]] = cell.Timestamp;
        } else if ('Bytes' in cell) {
          obj[colNames[col]] = cell.Bytes;
        }
      }
    }
    rows.push(obj);
  }

  return JSON.stringify(rows, null, 2);
}
