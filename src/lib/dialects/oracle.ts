import type { SqlDialect, ColumnDraft, ColumnChanges, IndexDraft, ForeignKeyDraft, TriggerDraft, PartitionDraft } from './types';
import type { CellValue, ColumnInfo } from '$lib/types';

export const oracleDialect: SqlDialect = {
  // Identifiers
  quoteIdent(name: string): string {
    // Oracle uses double quotes for quoted identifiers
    return `"${name}"`;
  },

  qualifiedTable(schema: string, table: string): string {
    return `${schema}.${table}`;
  },

  // Table operations
  dropTable(schema: string, table: string): string {
    return `DROP TABLE ${schema}.${table} PURGE`;
  },

  truncateTable(schema: string, table: string): string {
    return `TRUNCATE TABLE ${schema}.${table}`;
  },

  duplicateTable(schema: string, src: string, dst: string, mode: 'structure' | 'data'): string {
    if (mode === 'structure') {
      return `CREATE TABLE ${schema}.${dst} AS SELECT * FROM ${schema}.${src} WHERE 1=0`;
    } else {
      return `CREATE TABLE ${schema}.${dst} AS SELECT * FROM ${schema}.${src}`;
    }
  },

  refreshMaterializedView(schema: string, view: string): string | null {
    return `DBMS_MVIEW.REFRESH('${schema}.${view}', 'C')`;
  },

  // Cell literals
  cellLiteral(cell: CellValue, dataType?: string): string {
    if (cell === 'Null') {
      return 'NULL';
    } else if ('Bool' in cell) {
      return cell.Bool ? '1' : '0';
    } else if ('Int' in cell) {
      return cell.Int.toString();
    } else if ('Float' in cell) {
      return cell.Float.toString();
    } else if ('Text' in cell) {
      // Escape single quotes by doubling them
      return `'${cell.Text.replace(/'/g, "''")}'`;
    } else if ('Bytes' in cell) {
      // Convert bytes to hex string
      const hex = Array.from(cell.Bytes)
        .map((b: number) => b.toString(16).padStart(2, '0'))
        .join('');
      return `UTL_RAW.CAST_TO_VARCHAR2(HEXTORAW('${hex}'))`;
    } else if ('Json' in cell) {
      // Escape single quotes in JSON
      return `'${JSON.stringify(cell.Json).replace(/'/g, "''")}'`;
    } else if ('Timestamp' in cell) {
      return `TO_TIMESTAMP('${cell.Timestamp}', 'YYYY-MM-DD HH24:MI:SS.FF6')`;
    } else {
      return `'${cell}'`;
    }
  },

  // DDL generation
  addColumn(schema: string, table: string, col: ColumnDraft): string {
    const nullable = col.nullable ? '' : ' NOT NULL';
    const defaultValue = col.defaultValue ? ` DEFAULT ${col.defaultValue}` : '';
    const checkClause = col.check ? ` CHECK (${col.check})` : '';
    return `ALTER TABLE ${schema}.${table} ADD (${col.name} ${col.type}${nullable}${defaultValue}${checkClause})`;
  },

  alterColumn(schema: string, table: string, colName: string, changes: ColumnChanges): string {
    const clauses: string[] = [];
    
    if (changes.type) {
      clauses.push(`MODIFY ${colName} ${changes.type}`);
    }
    
    if (changes.nullable !== undefined) {
      clauses.push(changes.nullable ? `MODIFY ${colName} NULL` : `MODIFY ${colName} NOT NULL`);
    }
    
    if (changes.defaultValue !== undefined) {
      if (changes.defaultValue === null) {
        clauses.push(`DROP DEFAULT`);
      } else {
        clauses.push(`MODIFY ${colName} DEFAULT ${changes.defaultValue}`);
      }
    }

    if (changes.rename) {
      clauses.push(`RENAME COLUMN ${colName} TO ${changes.rename}`);
    }

    return clauses.map((clause: string) => `ALTER TABLE ${schema}.${table} ${clause}`).join(';\n');
  },

  dropColumn(schema: string, table: string, colName: string): string {
    return `ALTER TABLE ${schema}.${table} DROP COLUMN ${colName}`;
  },

  createIndex(schema: string, table: string, idx: IndexDraft): string {
    const unique = idx.unique ? 'UNIQUE ' : '';
    return `CREATE ${unique}INDEX ${idx.name} ON ${schema}.${table} (${idx.columns.join(', ')})`;
  },

  dropIndex(schema: string, indexName: string): string {
    return `DROP INDEX ${schema}.${indexName}`;
  },

  addForeignKey(schema: string, table: string, fk: ForeignKeyDraft): string {
    const name = fk.name || `fk_${table}_${fk.refTable}`;
    return `ALTER TABLE ${schema}.${table} ADD CONSTRAINT ${name} FOREIGN KEY (${fk.columns.join(', ')}) REFERENCES ${fk.refSchema}.${fk.refTable} (${fk.refColumns.join(', ')}) ON DELETE ${fk.onDelete} ON UPDATE ${fk.onUpdate}`;
  },

  dropConstraint(schema: string, table: string, constraintName: string): string {
    return `ALTER TABLE ${schema}.${table} DROP CONSTRAINT ${constraintName}`;
  },

  createTrigger(schema: string, table: string, trig: TriggerDraft): string | null {
    const condition = trig.condition ? `WHEN (${trig.condition})` : '';
    return `CREATE OR REPLACE TRIGGER ${trig.name}
${trig.timing} ${trig.event} ON ${schema}.${table}
FOR EACH ${trig.forEach}
${condition}
BEGIN
    ${trig.functionName}
END;`;
  },

  dropTrigger(schema: string, table: string, triggerName: string): string {
    return `DROP TRIGGER ${schema}.${triggerName}`;
  },

  toggleTrigger(schema: string, table: string, triggerName: string, enable: boolean): string | null {
    return `ALTER TRIGGER ${schema}.${triggerName} ${enable ? 'ENABLE' : 'DISABLE'}`;
  },

  addPartition(schema: string, parentTable: string, part: PartitionDraft): string | null {
    return `ALTER TABLE ${schema}.${parentTable} ADD PARTITION ${part.name} VALUES (${part.forValues})`;
  },

  detachPartition(schema: string, parentTable: string, partitionName: string): string | null {
    return `ALTER TABLE ${schema}.${parentTable} DROP PARTITION ${partitionName}`;
  },

  // Table creation
  createTable(schema: string, name: string, columns: ColumnDraft[]): string {
    const columnDefs = columns.map((col: ColumnDraft) => {
      const nullable = col.nullable ? '' : ' NOT NULL';
      const defaultValue = col.defaultValue ? ` DEFAULT ${col.defaultValue}` : '';
      const checkClause = col.check ? ` CHECK (${col.check})` : '';
      return `    ${col.name} ${col.type}${nullable}${defaultValue}${checkClause}`;
    });

    const primaryKeys = columns.filter((col: ColumnDraft) => col.primaryKey);
    const pkConstraint = primaryKeys.length > 0 
      ? `,\n    CONSTRAINT pk_${name} PRIMARY KEY (${primaryKeys.map((pk: ColumnDraft) => pk.name).join(', ')})`
      : '';

    return `CREATE TABLE ${schema}.${name} (\n${columnDefs.join(',\n')}${pkConstraint}\n)`;
  },

  // View creation
  createView(schema: string, name: string, sql: string, orReplace: boolean): string {
    const replace = orReplace ? 'OR REPLACE ' : '';
    return `CREATE ${replace}VIEW ${schema}.${name} AS ${sql}`;
  },

  createMaterializedView(schema: string, name: string, sql: string): string {
    return `CREATE MATERIALIZED VIEW ${schema}.${name} AS ${sql}`;
  },

  // Function creation
  createFunction(schema: string, name: string, params: string, returnType: string, language: string, body: string, orReplace: boolean): string {
    const replace = orReplace ? 'OR REPLACE ' : '';
    return `CREATE ${replace}FUNCTION ${schema}.${name}(${params}) RETURN ${returnType} IS
BEGIN
    ${body}
END;`;
  },

  // Sequence creation
  createSequence(schema: string, name: string, opts: { increment?: number; start?: number; min?: number; max?: number; cache?: number; cycle?: boolean }): string {
    const clauses: string[] = [];
    
    if (opts.increment !== undefined) clauses.push(`INCREMENT BY ${opts.increment}`);
    if (opts.start !== undefined) clauses.push(`START WITH ${opts.start}`);
    if (opts.min !== undefined) clauses.push(`MINVALUE ${opts.min}`);
    if (opts.max !== undefined) clauses.push(`MAXVALUE ${opts.max}`);
    if (opts.cache !== undefined) clauses.push(`CACHE ${opts.cache}`);
    if (opts.cycle) clauses.push('CYCLE');

    const clausesStr = clauses.length > 0 ? ` ${clauses.join(' ')}` : '';
    return `CREATE SEQUENCE ${schema}.${name}${clausesStr}`;
  },

  alterSequence(schema: string, name: string, opts: { increment?: number; min?: number; max?: number; cache?: number; cycle?: boolean; restart?: number }): string {
    const clauses: string[] = [];
    
    if (opts.increment !== undefined) clauses.push(`INCREMENT BY ${opts.increment}`);
    if (opts.min !== undefined) clauses.push(`MINVALUE ${opts.min}`);
    if (opts.max !== undefined) clauses.push(`MAXVALUE ${opts.max}`);
    if (opts.cache !== undefined) clauses.push(`CACHE ${opts.cache}`);
    if (opts.cycle !== undefined) clauses.push(opts.cycle ? 'CYCLE' : 'NOCYCLE');
    if (opts.restart !== undefined) clauses.push(`RESTART START WITH ${opts.restart}`);

    const clausesStr = clauses.length > 0 ? ` ${clauses.join(' ')}` : '';
    return `ALTER SEQUENCE ${schema}.${name}${clausesStr}`;
  },

  // Schema operations
  createSchema(schemaName: string): string {
    return `CREATE USER ${schemaName}`;
  },

  renameSchema(oldName: string, newName: string): string {
    return `ALTER USER ${oldName} RENAME TO ${newName}`;
  },

  dropSchema(schemaName: string, cascade: boolean): string {
    return `DROP USER ${schemaName}${cascade ? ' CASCADE' : ''}`;
  },

  dropView(schema: string, view: string, cascade: boolean): string {
    return `DROP VIEW ${schema}.${view}${cascade ? ' CASCADE' : ''}`;
  },

  dropMaterializedView(schema: string, view: string, cascade: boolean): string {
    return `DROP MATERIALIZED VIEW ${schema}.${view}${cascade ? ' CASCADE' : ''}`;
  },

  dropFunction(schema: string, name: string, argTypes: string | null, cascade: boolean): string {
    return `DROP FUNCTION ${schema}.${name}${argTypes ? `(${argTypes})` : ''}${cascade ? ' CASCADE' : ''}`;
  },

  dropSequence(schema: string, name: string, cascade: boolean): string {
    return `DROP SEQUENCE ${schema}.${name}${cascade ? ' CASCADE' : ''}`;
  },

  dropIndexCascade(schema: string, name: string, cascade: boolean): string {
    return `DROP INDEX ${schema}.${name}${cascade ? ' CASCADE' : ''}`;
  },

  dropForeignTable(schema: string, name: string, cascade: boolean): string {
    throw new Error('Oracle does not support foreign tables');
  },

  reindex(schema: string, name: string): string | null {
    return `ALTER INDEX ${schema}.${name} REBUILD`;
  },

  resetSequence(schema: string, name: string): string | null {
    return null; // Oracle doesn't have a direct reset sequence command
  },

  generateTemplate(objectType: 'table' | 'view' | 'materialized_view' | 'function' | 'sequence' | 'index' | 'schema', schemaName?: string): string {
    switch (objectType) {
      case 'table':
        return `CREATE TABLE ${schemaName || 'schema'}.table_name (
    id NUMBER GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    column_name VARCHAR2(100),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);`;
      case 'view':
        return `CREATE OR REPLACE VIEW ${schemaName || 'schema'}.view_name AS
SELECT column1, column2
FROM ${schemaName || 'schema'}.table_name
WHERE condition = 'value';`;
      case 'materialized_view':
        return `CREATE MATERIALIZED VIEW ${schemaName || 'schema'}.mv_name AS
SELECT column1, column2
FROM ${schemaName || 'schema'}.table_name
WHERE condition = 'value';`;
      case 'function':
        return `CREATE OR REPLACE FUNCTION ${schemaName || 'schema'}.function_name(
    param1 IN VARCHAR2,
    param2 IN NUMBER
) RETURN VARCHAR2 IS
BEGIN
    RETURN 'Result: ' || param1 || ', ' || param2;
END;`;
      case 'sequence':
        return `CREATE SEQUENCE ${schemaName || 'schema'}.sequence_name
    START WITH 1
    INCREMENT BY 1
    NOCACHE
    NOCYCLE;`;
      case 'index':
        return `CREATE INDEX index_name ON ${schemaName || 'schema'}.table_name (column_name);`;
      case 'schema':
        return `CREATE USER schema_name
    IDENTIFIED BY password
    DEFAULT TABLESPACE users
    TEMPORARY TABLESPACE temp;`;
      default:
        return '';
    }
  },

  // Editor integration
  codemirrorDialect(): any {
    // Return Oracle dialect for CodeMirror
    return null; // Will be implemented when @codemirror/lang-sql adds Oracle support
  },

  formatterLanguage(): any {
    // Return Oracle formatter for sql-formatter
    return null; // Will be implemented when sql-formatter adds Oracle support
  },

  explainAnalyzeQuery(sql: string, json: boolean): string | null {
    return `EXPLAIN PLAN FOR ${sql}`;
  },

  // Profiling
  statsQuery(schema: string, table: string, col: ColumnInfo): string | null {
    return `SELECT 
        COUNT(*) as total_count,
        COUNT(DISTINCT ${col.name}) as unique_count,
        COUNT(*) - COUNT(DISTINCT ${col.name}) as duplicate_count
    FROM ${schema}.${table}`;
  },

  histogramQuery(schema: string, table: string, col: ColumnInfo): string | null {
    return `SELECT 
        ${col.name} as value,
        COUNT(*) as frequency
    FROM ${schema}.${table}
    WHERE ${col.name} IS NOT NULL
    GROUP BY ${col.name}
    ORDER BY frequency DESC
    FETCH FIRST 20 ROWS ONLY`;
  },

  uniqueCountQuery(schema: string, table: string, col: ColumnInfo): string | null {
    return `SELECT COUNT(DISTINCT ${col.name}) as unique_count
    FROM ${schema}.${table}`;
  },

  bulkStatsQuery(schema: string, table: string, columns: ColumnInfo[]): string | null {
    const selects = columns.map((col: ColumnInfo) => 
        `COUNT(DISTINCT ${col.name}) as ${col.name}_unique_count`
    ).join(',\n        ');
    
    return `SELECT 
        ${selects}
    FROM ${schema}.${table}`;
  },

  bulkHistogramQuery(schema: string, table: string, columns: ColumnInfo[]): string | null {
    // Oracle doesn't easily support bulk histograms in a single query
    return null;
  },

  bulkUniqueCountQuery(schema: string, table: string, columns: ColumnInfo[]): string | null {
    const selects = columns.map((col: ColumnInfo) => 
        `COUNT(DISTINCT ${col.name}) as ${col.name}_unique_count`
    ).join(',\n        ');
    
    return `SELECT 
        ${selects}
    FROM ${schema}.${table}`;
  }
};
