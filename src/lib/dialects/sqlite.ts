import type { CellValue, ColumnInfo } from '$lib/types';
import type { SqlDialect, ColumnDraft, ColumnChanges, IndexDraft, ForeignKeyDraft, TriggerDraft, PartitionDraft } from './types';
import { SQLite } from '@codemirror/lang-sql';

function q(name: string): string {
  return `"${name.replace(/"/g, '""')}"`;
}

function qualified(schema: string, table: string): string {
  return schema ? `${q(schema)}.${q(table)}` : q(table);
}

export const sqliteDialect: SqlDialect = {
  quoteIdent: q,
  qualifiedTable: qualified,

  // -- Schema & object lifecycle -- SQLite has limited support --

  createSchema() {
    return '-- SQLite does not support CREATE SCHEMA.';
  },

  renameSchema() {
    return '-- SQLite does not support ALTER SCHEMA.';
  },

  dropSchema() {
    return '-- SQLite does not support DROP SCHEMA.';
  },

  dropView(_schema, view) {
    return `DROP VIEW IF EXISTS ${q(view)};`;
  },

  dropMaterializedView() {
    return '-- SQLite does not support materialized views.';
  },

  dropFunction() {
    return '-- SQLite does not support user-defined functions.';
  },

  dropSequence() {
    return '-- SQLite does not support sequences.';
  },

  dropIndexCascade(_schema, name) {
    return `DROP INDEX IF EXISTS ${q(name)};`;
  },

  dropForeignTable() {
    return '-- SQLite does not support foreign tables.';
  },

  reindex(_schema, name) {
    return `REINDEX ${q(name)};`;
  },

  resetSequence() {
    return null;
  },

  generateTemplate(objectType) {
    switch (objectType) {
      case 'schema':
        return '-- SQLite does not support schemas.\n';
      case 'table':
        return `CREATE TABLE new_table (\n    id INTEGER PRIMARY KEY AUTOINCREMENT,\n    name TEXT NOT NULL,\n    created_at TEXT DEFAULT (datetime('now'))\n);\n`;
      case 'view':
        return `CREATE VIEW new_view AS\nSELECT *\nFROM table_name\nWHERE 1;\n`;
      case 'materialized_view':
        return '-- SQLite does not support materialized views.\n';
      case 'function':
        return '-- SQLite does not support user-defined functions.\n';
      case 'sequence':
        return '-- SQLite does not support sequences.\n';
      case 'index':
        return `CREATE INDEX new_index\n    ON table_name (column_name);\n`;
    }
  },

  // -- Editor integration --

  codemirrorDialect() { return SQLite; },
  formatterLanguage() { return 'sqlite'; },
  explainAnalyzeQuery() { return null; },

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
    let typeSql = col.type;
    if (col.precision) typeSql += `(${col.precision})`;

    let sql = `ALTER TABLE ${qualified(schema, table)} ADD COLUMN ${q(col.name)} ${typeSql}`;
    if (col.primaryKey) sql += ' PRIMARY KEY';
    if (col.unique && !col.primaryKey) sql += ' UNIQUE';
    if (!col.nullable && !col.primaryKey) sql += ' NOT NULL';
    if (col.defaultValue) sql += ` DEFAULT ${col.defaultValue}`;
    if (col.check) sql += ` CHECK (${col.check})`;
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
