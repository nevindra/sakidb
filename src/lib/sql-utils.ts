import type { CellValue, ColumnDef, ColumnInfo } from '$lib/types';

/** Escape a SQL identifier (column/table/schema name) by doubling embedded quotes. */
export function quoteIdent(name: string): string {
  return `"${name.replace(/"/g, '""')}"`;
}

/** Build a quoted table reference, omitting schema when empty (e.g. SQLite). */
export function qualifiedTable(schema: string, table: string): string {
  return schema ? `${quoteIdent(schema)}.${quoteIdent(table)}` : quoteIdent(table);
}

// Types that need explicit SQL casts when used as literals
const SQL_CAST_MAP: Record<string, string> = {
  uuid: 'uuid', inet: 'inet', cidr: 'cidr',
  macaddr: 'macaddr', macaddr8: 'macaddr8',
  date: 'date', time: 'time', timetz: 'timetz',
  interval: 'interval', xml: 'xml',
  point: 'point', line: 'line', lseg: 'lseg',
  box: 'box', circle: 'circle', polygon: 'polygon', path: 'path',
  tsvector: 'tsvector', tsquery: 'tsquery',
  bit: 'bit', varbit: 'varbit',
  pg_lsn: 'pg_lsn',
};

export function cellValueToSqlLiteral(cell: CellValue, dataType?: string): string {
  if (cell === 'Null') return 'NULL';
  if ('Bool' in cell) return cell.Bool ? 'TRUE' : 'FALSE';
  if ('Int' in cell) return String(cell.Int);
  if ('Float' in cell) return Number.isFinite(cell.Float) ? String(cell.Float) : 'NULL';
  if ('Text' in cell) {
    const escaped = `'${cell.Text.replace(/'/g, "''")}'`;
    if (dataType) {
      const t = dataType.toLowerCase().replace(/\s*\(.*\)$/, '');
      const cast = SQL_CAST_MAP[t];
      if (cast) return `${escaped}::${cast}`;
    }
    return escaped;
  }
  if ('Json' in cell) return `'${cell.Json.replace(/'/g, "''")}'::jsonb`;
  if ('Timestamp' in cell) {
    const escaped = `'${cell.Timestamp.replace(/'/g, "''")}'`;
    if (dataType) {
      const t = dataType.toLowerCase();
      if (t === 'date') return `${escaped}::date`;
      if (t === 'timetz' || t === 'time with time zone') return `${escaped}::timetz`;
      if (t === 'time' || t === 'time without time zone') return `${escaped}::time`;
      if (t.startsWith('timestamptz') || t.startsWith('timestamp with')) return `${escaped}::timestamptz`;
    }
    return `${escaped}::timestamp`;
  }
  if ('Bytes' in cell) {
    const hex = cell.Bytes.map(b => b.toString(16).padStart(2, '0')).join('');
    return `'\\x${hex}'::bytea`;
  }
  return 'NULL';
}

function buildWhereClause(
  pkColumns: string[],
  pkValues: CellValue[],
  pkDataTypes?: string[],
): string {
  return pkColumns
    .map((col, i) => `${quoteIdent(col)} = ${cellValueToSqlLiteral(pkValues[i], pkDataTypes?.[i])}`)
    .join(' AND ');
}

export function generateUpdateSql(
  schema: string,
  table: string,
  pkColumns: string[],
  pkValues: CellValue[],
  changes: [string, CellValue, string?][],
  pkDataTypes?: string[],
): string {
  const setClauses = changes.map(
    ([col, val, dt]) => `${quoteIdent(col)} = ${cellValueToSqlLiteral(val, dt)}`
  );
  return `UPDATE ${qualifiedTable(schema, table)} SET ${setClauses.join(', ')} WHERE ${buildWhereClause(pkColumns, pkValues, pkDataTypes)}`;
}

export function generateInsertSql(
  schema: string,
  table: string,
  columns: string[],
  values: CellValue[],
  dataTypes?: string[],
): string {
  const nonNullPairs = columns
    .map((col, i) => ({ col, val: values[i], dt: dataTypes?.[i] }))
    .filter(p => p.val !== 'Null');
  if (nonNullPairs.length === 0) {
    return `INSERT INTO ${qualifiedTable(schema, table)} DEFAULT VALUES`;
  }
  const colList = nonNullPairs.map(p => quoteIdent(p.col)).join(', ');
  const valList = nonNullPairs.map(p => cellValueToSqlLiteral(p.val, p.dt)).join(', ');
  return `INSERT INTO ${qualifiedTable(schema, table)} (${colList}) VALUES (${valList})`;
}

export function generateDeleteSql(
  schema: string,
  table: string,
  pkColumns: string[],
  pkValues: CellValue[],
  pkDataTypes?: string[],
): string {
  return `DELETE FROM ${qualifiedTable(schema, table)} WHERE ${buildWhereClause(pkColumns, pkValues, pkDataTypes)}`;
}

export function wrapInTransaction(statements: string[]): string {
  return `BEGIN;\n${statements.join(';\n')};\nCOMMIT;`;
}

export function parseInputToCellValue(input: string, dataType: string): CellValue {
  const trimmed = input.trim();
  if (trimmed === '' || trimmed.toLowerCase() === 'null') return 'Null';

  const t = dataType.toLowerCase();
  const tBase = t.replace(/\s*\(.*\)$/, '');

  if (tBase === 'bool' || tBase === 'boolean') {
    const lower = trimmed.toLowerCase();
    return { Bool: lower === 'true' || lower === 't' || lower === '1' || lower === 'yes' };
  }
  if (['int2', 'int4', 'int8', 'smallint', 'integer', 'bigint', 'oid',
       'smallserial', 'serial', 'bigserial', 'serial2', 'serial4', 'serial8'].includes(tBase)) {
    const n = parseInt(trimmed, 10);
    return isNaN(n) ? 'Null' : { Int: n };
  }
  if (['float4', 'float8', 'real', 'double precision', 'numeric', 'decimal', 'money'].includes(tBase)) {
    // Strip currency symbols for money type
    const cleaned = tBase === 'money' ? trimmed.replace(/[$€£¥,]/g, '') : trimmed;
    const n = parseFloat(cleaned);
    return isNaN(n) ? 'Null' : { Float: n };
  }
  if (['json', 'jsonb'].includes(tBase)) {
    return { Json: trimmed };
  }
  if (['timestamp', 'timestamptz',
       'timestamp without time zone', 'timestamp with time zone'].includes(t)) {
    return { Timestamp: trimmed };
  }
  if (['date'].includes(tBase)) {
    return { Timestamp: trimmed };
  }
  if (['time', 'timetz', 'time without time zone', 'time with time zone'].includes(t)) {
    return { Timestamp: trimmed };
  }
  if (tBase === 'interval') {
    return { Text: trimmed };
  }
  // All other types stored as Text — PG validates on write
  return { Text: trimmed };
}

export function cellValueToEditText(v: CellValue): string {
  if (v === 'Null') return '';
  if ('Bool' in v) return String(v.Bool);
  if ('Int' in v) return String(v.Int);
  if ('Float' in v) return String(v.Float);
  if ('Text' in v) return v.Text;
  if ('Json' in v) return v.Json;
  if ('Timestamp' in v) return v.Timestamp;
  if ('Bytes' in v) return '';
  return '';
}

export function getPkColumnIndices(
  columns: ColumnDef[],
  columnInfos: ColumnInfo[],
): number[] {
  const pkNames = new Set(
    columnInfos.filter(c => c.is_primary_key).map(c => c.name)
  );
  return columns
    .map((col, i) => pkNames.has(col.name) ? i : -1)
    .filter(i => i >= 0);
}

export function cellValueEquals(a: CellValue, b: CellValue): boolean {
  if (a === 'Null') return b === 'Null';
  if (b === 'Null') return false;
  if ('Bool' in a) return 'Bool' in b && a.Bool === b.Bool;
  if ('Int' in a) return 'Int' in b && a.Int === b.Int;
  if ('Float' in a) return 'Float' in b && a.Float === b.Float;
  if ('Text' in a) return 'Text' in b && a.Text === b.Text;
  if ('Json' in a) return 'Json' in b && a.Json === b.Json;
  if ('Timestamp' in a) return 'Timestamp' in b && a.Timestamp === b.Timestamp;
  if ('Bytes' in a) {
    if (!('Bytes' in b) || a.Bytes.length !== b.Bytes.length) return false;
    for (let i = 0; i < a.Bytes.length; i++) {
      if (a.Bytes[i] !== b.Bytes[i]) return false;
    }
    return true;
  }
  return false;
}
