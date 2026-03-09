import type { CellValue, ColumnInfo } from '$lib/types';
import type { SqlDialect, ColumnDraft, ColumnChanges, IndexDraft, ForeignKeyDraft, TriggerDraft, PartitionDraft } from './types';

function q(name: string): string {
  return `"${name.replace(/"/g, '""')}"`;
}

function qualified(schema: string, table: string): string {
  return schema ? `${q(schema)}.${q(table)}` : q(table);
}

export const sqliteDialect: SqlDialect = {
  quoteIdent: q,
  qualifiedTable: qualified,

  dropTable(schema, table) {
    return `DROP TABLE ${qualified(schema, table)};`;
  },

  truncateTable(schema, table) {
    return `DELETE FROM ${qualified(schema, table)};`;
  },

  duplicateTable(schema, src, dst, mode) {
    const s = qualified(schema, src);
    const d = qualified(schema, dst);
    return mode === 'structure'
      ? `CREATE TABLE ${d} AS SELECT * FROM ${s} WHERE 0;`
      : `CREATE TABLE ${d} AS SELECT * FROM ${s};`;
  },

  refreshMaterializedView() {
    return null;
  },

  cellLiteral(cell: CellValue, _dataType?: string): string {
    if (cell === 'Null') return 'NULL';
    if ('Bool' in cell) return cell.Bool ? '1' : '0';
    if ('Int' in cell) return String(cell.Int);
    if ('Float' in cell) return Number.isFinite(cell.Float) ? String(cell.Float) : 'NULL';
    if ('Text' in cell) return `'${cell.Text.replace(/'/g, "''")}'`;
    if ('Json' in cell) return `'${cell.Json.replace(/'/g, "''")}'`;
    if ('Timestamp' in cell) return `'${cell.Timestamp.replace(/'/g, "''")}'`;
    if ('Bytes' in cell) {
      const hex = cell.Bytes.map((b: number) => b.toString(16).padStart(2, '0')).join('');
      return `X'${hex}'`;
    }
    return 'NULL';
  },

  // -- DDL -- SQLite supports a subset --

  addColumn(schema, table, col) {
    let sql = `ALTER TABLE ${qualified(schema, table)} ADD COLUMN ${q(col.name)} ${col.type}`;
    if (!col.nullable) sql += ' NOT NULL';
    if (col.defaultValue) sql += ` DEFAULT ${col.defaultValue}`;
    return sql + ';';
  },

  alterColumn(_schema, _table, _colName, _changes) {
    return '-- SQLite does not support ALTER COLUMN. Rebuild the table instead.';
  },

  dropColumn(schema, table, colName) {
    return `ALTER TABLE ${qualified(schema, table)} DROP COLUMN ${q(colName)};`;
  },

  createIndex(schema, table, idx) {
    const unique = idx.unique ? 'UNIQUE ' : '';
    const cols = idx.columns.map(q).join(', ');
    return `CREATE ${unique}INDEX ${q(idx.name)} ON ${qualified(schema, table)} (${cols});`;
  },

  dropIndex(_schema, indexName) {
    return `DROP INDEX ${q(indexName)};`;
  },

  addForeignKey(_schema, _table, _fk) {
    return '-- SQLite does not support adding foreign keys after table creation.';
  },

  dropConstraint(_schema, _table, _constraintName) {
    return '-- SQLite does not support DROP CONSTRAINT.';
  },

  createTrigger(_schema, _table, _trig) {
    return null;
  },

  dropTrigger(_schema, _table, triggerName) {
    return `DROP TRIGGER ${q(triggerName)};`;
  },

  toggleTrigger(_schema, _table, _triggerName, _enable) {
    return null;
  },

  addPartition(_schema, _parentTable, _part) {
    return null;
  },

  detachPartition(_schema, _parentTable, _partitionName) {
    return null;
  },

  // -- Profiling -- return null; SQLite lacks FILTER, PERCENTILE_CONT --

  statsQuery() { return null; },
  histogramQuery() { return null; },
  uniqueCountQuery() { return null; },
  bulkStatsQuery() { return null; },
  bulkHistogramQuery() { return null; },
  bulkUniqueCountQuery() { return null; },
};
