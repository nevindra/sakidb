import type { ColumnDef, CellValue, ColumnArray, TextColumn } from '$lib/types';
import { getCategoryCss } from '$lib/type-utils';

/**
 * Class wrapper for query results that prevents Svelte 5 from
 * deep-proxying the cells array. Svelte only proxies plain objects
 * and arrays — class instances are left as-is.
 */
export class QueryResultData {
  readonly columns: ColumnDef[];
  readonly cells: CellValue[];
  readonly row_count: number;
  readonly execution_time_ms: number;
  readonly truncated: boolean;

  constructor(
    columns: ColumnDef[],
    cells: CellValue[],
    row_count: number,
    execution_time_ms: number,
    truncated: boolean,
  ) {
    this.columns = columns;
    this.cells = cells;
    this.row_count = row_count;
    this.execution_time_ms = execution_time_ms;
    this.truncated = truncated;
  }
}

/**
 * Columnar result data — stores values in typed arrays per column
 * instead of a flat CellValue[] array. This reduces JS heap usage
 * by 5-8x for numeric data.
 *
 * Class instance prevents Svelte 5 from deep-proxying the typed arrays.
 */
export class ColumnarResultData {
  readonly columns: ColumnDef[];
  readonly columnData: ColumnArray[];
  readonly row_count: number;
  readonly execution_time_ms: number;
  readonly truncated: boolean;
  private _decoder = new TextDecoder();

  constructor(
    columns: ColumnDef[],
    columnData: ColumnArray[],
    row_count: number,
    execution_time_ms: number,
    truncated: boolean,
  ) {
    this.columns = columns;
    this.columnData = columnData;
    this.row_count = row_count;
    this.execution_time_ms = execution_time_ms;
    this.truncated = truncated;
  }

  /** Lazily decode a text cell from raw UTF-8 bytes. Caches in values[row]. */
  private _getText(cd: TextColumn, row: number): string {
    let s = cd.values[row];
    if (s === undefined) {
      const start = cd._offsets[row];
      const end = cd._offsets[row + 1];
      s = this._decoder.decode(cd._raw.subarray(start, end));
      cd.values[row] = s;
    }
    return s;
  }

  /** Get the byte length of a text cell without decoding the string. */
  getTextByteLength(row: number, col: number): number {
    const cd = this.columnData[col];
    if (cd.type !== 'text' || cd.nulls[row] !== 0) return 0;
    return cd._offsets[row + 1] - cd._offsets[row];
  }

  /** Check if a cell is null. */
  isNull(row: number, col: number): boolean {
    return this.columnData[col].nulls[row] !== 0;
  }

  /** Get the raw typed value for a cell. Returns undefined for null. */
  getValue(row: number, col: number): number | string | Uint8Array | boolean | undefined {
    const cd = this.columnData[col];
    if (cd.nulls[row] !== 0) return undefined;
    if (cd.type === 'bool') return cd.values[row] !== 0;
    if (cd.type === 'text') return this._getText(cd, row);
    return cd.values[row];
  }

  /**
   * Construct a legacy CellValue on demand.
   * Used by components that still need the tagged union (CellEditor, copy-utils, sql-utils).
   * Only call this for single cells — NOT in hot loops.
   */
  toCellValue(row: number, col: number): CellValue {
    const cd = this.columnData[col];
    if (cd.nulls[row] !== 0) return 'Null';
    switch (cd.type) {
      case 'number': {
        const v = cd.values[row];
        return Number.isInteger(v) ? { Int: v } : { Float: v };
      }
      case 'bool':
        return { Bool: cd.values[row] !== 0 };
      case 'text': {
        const s = this._getText(cd, row);
        const dt = this.columns[col].data_type.toLowerCase();
        if (dt === 'json' || dt === 'jsonb') return { Json: s };
        if (dt.startsWith('timestamp') || dt === 'date' || dt.startsWith('time') || dt === 'interval')
          return { Timestamp: s };
        return { Text: s };
      }
      case 'bytes':
        return { Bytes: Array.from(cd.values[row]) };
    }
  }

  /** Get sort key for a cell — fast path without creating CellValue. */
  getSortKey(row: number, col: number): string | number {
    const cd = this.columnData[col];
    if (cd.nulls[row] !== 0) return '';
    if (cd.type === 'number') return cd.values[row];
    if (cd.type === 'bool') return cd.values[row];
    if (cd.type === 'text') return this._getText(cd, row);
    return ''; // bytes not sortable
  }

  /** Get display text and CSS class for a cell — fast path for rendering. */
  getCellDisplay(row: number, col: number): { text: string; cls: string; isNull: boolean } {
    const cd = this.columnData[col];
    if (cd.nulls[row] !== 0) return { text: 'NULL', cls: 'text-text-dim italic', isNull: true };

    switch (cd.type) {
      case 'number':
        return { text: String(cd.values[row]), cls: 'text-right tabular-nums', isNull: false };
      case 'bool':
        return { text: String(cd.values[row] !== 0), cls: 'text-warning', isNull: false };
      case 'text': {
        const s = this._getText(cd, row);
        const dt = this.columns[col].data_type;
        const css = getCategoryCss(dt);
        const display = s.length > 200 ? s.slice(0, 200) + '\u2026' : s;
        return { text: display, cls: css, isNull: false };
      }
      case 'bytes': {
        const bytes = cd.values[row];
        const hex = Array.from(bytes.slice(0, 16))
          .map(b => b.toString(16).padStart(2, '0'))
          .join(' ');
        return {
          text: `\\x${hex}${bytes.length > 16 ? '...' : ''}`,
          cls: 'font-mono text-text-dim',
          isNull: false,
        };
      }
    }
  }
}
