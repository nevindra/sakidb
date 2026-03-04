import type { CellValue, ColumnDef, ColumnInfo } from '$lib/types';

export function cellValueToSqlLiteral(cell: CellValue): string {
  if (cell === 'Null') return 'NULL';
  if ('Bool' in cell) return cell.Bool ? 'TRUE' : 'FALSE';
  if ('Int' in cell) return String(cell.Int);
  if ('Float' in cell) return String(cell.Float);
  if ('Text' in cell) return `'${cell.Text.replace(/'/g, "''")}'`;
  if ('Json' in cell) return `'${cell.Json.replace(/'/g, "''")}'::jsonb`;
  if ('Timestamp' in cell) return `'${cell.Timestamp.replace(/'/g, "''")}'::timestamp`;
  if ('Bytes' in cell) {
    const hex = cell.Bytes.map(b => b.toString(16).padStart(2, '0')).join('');
    return `'\\x${hex}'::bytea`;
  }
  return 'NULL';
}

function buildWhereClause(
  pkColumns: string[],
  pkValues: CellValue[],
): string {
  return pkColumns
    .map((col, i) => `"${col}" = ${cellValueToSqlLiteral(pkValues[i])}`)
    .join(' AND ');
}

export function generateUpdateSql(
  schema: string,
  table: string,
  pkColumns: string[],
  pkValues: CellValue[],
  changes: [string, CellValue][],
): string {
  const setClauses = changes.map(
    ([col, val]) => `"${col}" = ${cellValueToSqlLiteral(val)}`
  );
  return `UPDATE "${schema}"."${table}" SET ${setClauses.join(', ')} WHERE ${buildWhereClause(pkColumns, pkValues)}`;
}

export function generateInsertSql(
  schema: string,
  table: string,
  columns: string[],
  values: CellValue[],
): string {
  const nonNullPairs = columns
    .map((col, i) => ({ col, val: values[i] }))
    .filter(p => p.val !== 'Null');
  if (nonNullPairs.length === 0) {
    return `INSERT INTO "${schema}"."${table}" DEFAULT VALUES`;
  }
  const colList = nonNullPairs.map(p => `"${p.col}"`).join(', ');
  const valList = nonNullPairs.map(p => cellValueToSqlLiteral(p.val)).join(', ');
  return `INSERT INTO "${schema}"."${table}" (${colList}) VALUES (${valList})`;
}

export function generateDeleteSql(
  schema: string,
  table: string,
  pkColumns: string[],
  pkValues: CellValue[],
): string {
  return `DELETE FROM "${schema}"."${table}" WHERE ${buildWhereClause(pkColumns, pkValues)}`;
}

export function wrapInTransaction(statements: string[]): string {
  return `BEGIN;\n${statements.join(';\n')};\nCOMMIT;`;
}

export function parseInputToCellValue(input: string, dataType: string): CellValue {
  const trimmed = input.trim();
  if (trimmed === '' || trimmed.toLowerCase() === 'null') return 'Null';

  const t = dataType.toLowerCase();

  if (t === 'bool') {
    const lower = trimmed.toLowerCase();
    return { Bool: lower === 'true' || lower === 't' || lower === '1' || lower === 'yes' };
  }
  if (['int2', 'int4', 'int8', 'oid'].includes(t)) {
    const n = parseInt(trimmed, 10);
    return isNaN(n) ? 'Null' : { Int: n };
  }
  if (['float4', 'float8', 'numeric'].includes(t)) {
    const n = parseFloat(trimmed);
    return isNaN(n) ? 'Null' : { Float: n };
  }
  if (['json', 'jsonb'].includes(t)) {
    return { Json: trimmed };
  }
  if (['timestamp', 'timestamptz'].includes(t)) {
    return { Timestamp: trimmed };
  }
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
