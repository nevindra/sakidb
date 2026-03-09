import type { CellValue, ColumnInfo } from '$lib/types';
import type { SqlDialect, ColumnDraft, ColumnChanges, IndexDraft, ForeignKeyDraft, TriggerDraft, PartitionDraft } from './types';
import { PostgreSQL } from '@codemirror/lang-sql';

// -- Helpers (private) --

function q(name: string): string {
  return `"${name.replace(/"/g, '""')}"`;
}

function qualified(schema: string, table: string): string {
  return schema ? `${q(schema)}.${q(table)}` : q(table);
}

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

type ColumnCategory = 'numeric' | 'text' | 'bool' | 'other';

function categorize(dataType: string): ColumnCategory {
  const t = dataType.toLowerCase();
  if (/^(smallint|integer|bigint|int[248]?|serial|bigserial|smallserial|numeric|decimal|real|double precision|float[48]?|money)/.test(t)) return 'numeric';
  if (/^(bool|boolean)/.test(t)) return 'bool';
  if (/^(char|varchar|character|text|citext|name|uuid|inet|macaddr|xml)/.test(t)) return 'text';
  return 'other';
}

function needsTextCast(cat: ColumnCategory): boolean {
  return cat === 'other';
}

// -- Dialect --

export const postgresDialect: SqlDialect = {
  quoteIdent: q,
  qualifiedTable: qualified,

  // -- Schema & object lifecycle --

  createSchema(schemaName) {
    return `CREATE SCHEMA ${q(schemaName)};`;
  },

  renameSchema(oldName, newName) {
    return `ALTER SCHEMA ${q(oldName)} RENAME TO ${q(newName)};`;
  },

  dropSchema(schemaName, cascade) {
    return `DROP SCHEMA ${q(schemaName)}${cascade ? ' CASCADE' : ''};`;
  },

  dropView(schema, view, cascade) {
    return `DROP VIEW ${qualified(schema, view)}${cascade ? ' CASCADE' : ''};`;
  },

  dropMaterializedView(schema, view, cascade) {
    return `DROP MATERIALIZED VIEW ${qualified(schema, view)}${cascade ? ' CASCADE' : ''};`;
  },

  dropFunction(schema, name, argTypes, cascade) {
    const sig = argTypes ? `${qualified(schema, name)}(${argTypes})` : `${qualified(schema, name)}`;
    return `DROP FUNCTION ${sig}${cascade ? ' CASCADE' : ''};`;
  },

  dropSequence(schema, name, cascade) {
    return `DROP SEQUENCE ${qualified(schema, name)}${cascade ? ' CASCADE' : ''};`;
  },

  dropIndexCascade(schema, name, cascade) {
    return `DROP INDEX ${qualified(schema, name)}${cascade ? ' CASCADE' : ''};`;
  },

  dropForeignTable(schema, name, cascade) {
    return `DROP FOREIGN TABLE ${qualified(schema, name)}${cascade ? ' CASCADE' : ''};`;
  },

  reindex(schema, name) {
    return `REINDEX INDEX ${qualified(schema, name)};`;
  },

  resetSequence(schema, name) {
    return `ALTER SEQUENCE ${qualified(schema, name)} RESTART WITH 1;`;
  },

  generateTemplate(objectType, schemaName) {
    const s = schemaName ? q(schemaName) : '"public"';
    switch (objectType) {
      case 'schema':
        return `CREATE SCHEMA new_schema;\n`;
      case 'table':
        return `CREATE TABLE ${s}.new_table (\n    id SERIAL PRIMARY KEY,\n    name VARCHAR(255) NOT NULL,\n    created_at TIMESTAMPTZ DEFAULT NOW()\n);\n`;
      case 'view':
        return `CREATE OR REPLACE VIEW ${s}.new_view AS\nSELECT *\nFROM ${s}.table_name\nWHERE true;\n`;
      case 'materialized_view':
        return `CREATE MATERIALIZED VIEW ${s}.new_materialized_view AS\nSELECT *\nFROM ${s}.table_name\nWHERE true;\n`;
      case 'function':
        return `CREATE OR REPLACE FUNCTION ${s}.new_function()\nRETURNS void\nLANGUAGE plpgsql\nAS $$\nBEGIN\n    -- function body\nEND;\n$$;\n`;
      case 'sequence':
        return `CREATE SEQUENCE ${s}.new_sequence\n    INCREMENT 1\n    START 1\n    MINVALUE 1\n    NO MAXVALUE\n    CACHE 1;\n`;
      case 'index':
        return `CREATE INDEX new_index\n    ON ${s}.table_name\n    USING btree (column_name);\n`;
    }
  },

  // -- Editor integration --

  codemirrorDialect() { return PostgreSQL; },
  formatterLanguage() { return 'postgresql'; },
  explainAnalyzeQuery(sql, json) {
    const format = json ? 'FORMAT JSON' : 'FORMAT TEXT';
    return `EXPLAIN (ANALYZE, BUFFERS, ${format}) ${sql}`;
  },

  // -- Table ops --

  dropTable(schema, table) {
    return `DROP TABLE ${qualified(schema, table)} CASCADE;`;
  },

  truncateTable(schema, table) {
    return `TRUNCATE TABLE ${qualified(schema, table)};`;
  },

  duplicateTable(schema, src, dst, mode) {
    const s = qualified(schema, src);
    const d = qualified(schema, dst);
    return mode === 'structure'
      ? `CREATE TABLE ${d} (LIKE ${s} INCLUDING ALL);`
      : `CREATE TABLE ${d} AS SELECT * FROM ${s};`;
  },

  refreshMaterializedView(schema, view) {
    return `REFRESH MATERIALIZED VIEW ${qualified(schema, view)};`;
  },

  // -- Cell literals --

  cellLiteral(cell: CellValue, dataType?: string): string {
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
      const hex = cell.Bytes.map((b: number) => b.toString(16).padStart(2, '0')).join('');
      return `'\\x${hex}'::bytea`;
    }
    return 'NULL';
  },

  // -- DDL --

  addColumn(schema, table, col) {
    let typeSql = col.type;
    if (col.precision) typeSql += `(${col.precision})`;
    if (col.isArray) typeSql += '[]';

    const stmts: string[] = [];
    let sql = `ALTER TABLE ${qualified(schema, table)} ADD COLUMN ${q(col.name)} ${typeSql}`;
    if (col.primaryKey) sql += ' PRIMARY KEY';
    if (col.unique && !col.primaryKey) sql += ' UNIQUE';
    if (!col.nullable && !col.primaryKey) sql += ' NOT NULL';
    if (col.defaultValue) sql += ` DEFAULT ${col.defaultValue}`;
    if (col.check) sql += ` CHECK (${col.check})`;
    stmts.push(sql + ';');

    if (col.comment) {
      stmts.push(`COMMENT ON COLUMN ${qualified(schema, table)}.${q(col.name)} IS '${col.comment.replace(/'/g, "''")}';`);
    }

    return stmts.join('\n');
  },

  alterColumn(schema, table, colName, changes) {
    const stmts: string[] = [];
    const target = qualified(schema, table);
    if (changes.type) stmts.push(`ALTER TABLE ${target} ALTER COLUMN ${q(colName)} TYPE ${changes.type};`);
    if (changes.nullable === false) stmts.push(`ALTER TABLE ${target} ALTER COLUMN ${q(colName)} SET NOT NULL;`);
    else if (changes.nullable === true) stmts.push(`ALTER TABLE ${target} ALTER COLUMN ${q(colName)} DROP NOT NULL;`);
    if (changes.defaultValue !== undefined) {
      if (changes.defaultValue === null) stmts.push(`ALTER TABLE ${target} ALTER COLUMN ${q(colName)} DROP DEFAULT;`);
      else stmts.push(`ALTER TABLE ${target} ALTER COLUMN ${q(colName)} SET DEFAULT ${changes.defaultValue};`);
    }
    if (changes.rename) stmts.push(`ALTER TABLE ${target} RENAME COLUMN ${q(colName)} TO ${q(changes.rename)};`);
    return stmts.join('\n');
  },

  dropColumn(schema, table, colName) {
    return `ALTER TABLE ${qualified(schema, table)} DROP COLUMN ${q(colName)};`;
  },

  createIndex(schema, table, idx) {
    const unique = idx.unique ? 'UNIQUE ' : '';
    const cols = idx.columns.map(q).join(', ');
    return `CREATE ${unique}INDEX ${q(idx.name)} ON ${qualified(schema, table)} USING ${idx.type} (${cols});`;
  },

  dropIndex(schema, indexName) {
    return `DROP INDEX ${qualified(schema, indexName)};`;
  },

  addForeignKey(schema, table, fk) {
    const constraintName = fk.name || `fk_${table}_${fk.columns.join('_')}`;
    const localCols = fk.columns.map(q).join(', ');
    const foreignCols = fk.refColumns.map(q).join(', ');
    return `ALTER TABLE ${qualified(schema, table)} ADD CONSTRAINT ${q(constraintName)} FOREIGN KEY (${localCols}) REFERENCES ${qualified(fk.refSchema, fk.refTable)} (${foreignCols}) ON UPDATE ${fk.onUpdate} ON DELETE ${fk.onDelete};`;
  },

  dropConstraint(schema, table, constraintName) {
    return `ALTER TABLE ${qualified(schema, table)} DROP CONSTRAINT ${q(constraintName)};`;
  },

  createTrigger(schema, table, trig) {
    let sql = `CREATE TRIGGER ${q(trig.name)} ${trig.timing} ${trig.event} ON ${qualified(schema, table)}\n    FOR EACH ${trig.forEach}`;
    if (trig.condition) sql += `\n    WHEN (${trig.condition})`;
    sql += `\n    EXECUTE FUNCTION ${qualified(trig.functionSchema, trig.functionName)}();`;
    return sql;
  },

  dropTrigger(schema, table, triggerName) {
    return `DROP TRIGGER ${q(triggerName)} ON ${qualified(schema, table)};`;
  },

  toggleTrigger(schema, table, triggerName, enable) {
    const action = enable ? 'ENABLE' : 'DISABLE';
    return `ALTER TABLE ${qualified(schema, table)} ${action} TRIGGER ${q(triggerName)};`;
  },

  addPartition(schema, parentTable, part) {
    return `CREATE TABLE ${qualified(schema, part.name)} PARTITION OF ${qualified(schema, parentTable)} ${part.forValues};`;
  },

  detachPartition(schema, parentTable, partitionName) {
    return `ALTER TABLE ${qualified(schema, parentTable)} DETACH PARTITION ${qualified(schema, partitionName)};`;
  },

  // -- Profiling --

  statsQuery(schema, table, col) {
    const tbl = qualified(schema, table);
    const c = q(col.name);
    const cat = categorize(col.data_type);
    const distinctExpr = needsTextCast(cat) ? `${c}::text` : c;
    const parts: string[] = [
      `COUNT(*) AS total_count`,
      `COUNT(*) - COUNT(${c}) AS null_count`,
      `COUNT(${c}) AS not_null_count`,
      `COUNT(DISTINCT ${distinctExpr}) AS distinct_count`,
    ];
    if (cat === 'numeric') {
      parts.push(
        `COUNT(*) FILTER (WHERE ${c} = 0) AS zero_count`,
        `COUNT(*) FILTER (WHERE ${c}::text = 'NaN') AS nan_count`,
        `MIN(${c})::text AS min_val`, `MAX(${c})::text AS max_val`,
        `AVG(${c}::double precision)::double precision AS avg_val`,
        `(PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY ${c}::double precision))::text AS median_val`,
      );
    } else if (cat === 'text') {
      parts.push(
        `0 AS zero_count`, `0 AS nan_count`,
        `MIN(${c})::text AS min_val`, `MAX(${c})::text AS max_val`,
        `AVG(LENGTH(${c}))::double precision AS avg_val`,
        `(PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY LENGTH(${c})))::text AS median_val`,
      );
    } else {
      parts.push(
        `0 AS zero_count`, `0 AS nan_count`,
        `MIN(${c}::text) AS min_val`, `MAX(${c}::text) AS max_val`,
        `NULL::double precision AS avg_val`, `NULL::text AS median_val`,
      );
    }
    return `SELECT ${parts.join(', ')} FROM ${tbl}`;
  },

  histogramQuery(schema, table, col) {
    const tbl = qualified(schema, table);
    const c = q(col.name);
    return `SELECT ${c}::text AS value, COUNT(*) AS freq FROM ${tbl} WHERE ${c} IS NOT NULL GROUP BY ${c}::text ORDER BY freq DESC LIMIT 20`;
  },

  uniqueCountQuery(schema, table, col) {
    const tbl = qualified(schema, table);
    const c = q(col.name);
    return `SELECT COUNT(*) AS unique_count FROM (SELECT ${c}::text FROM ${tbl} WHERE ${c} IS NOT NULL GROUP BY ${c}::text HAVING COUNT(*) = 1) sub`;
  },

  bulkStatsQuery(schema, table, columns) {
    const tbl = qualified(schema, table);
    const selects = columns.map(col => {
      const c = q(col.name);
      const cat = categorize(col.data_type);
      const distinctExpr = needsTextCast(cat) ? `${c}::text` : c;
      const alias = col.name.replace(/'/g, "''");
      const parts: string[] = [
        `'${alias}' AS col_name`, `COUNT(*) AS total_count`,
        `COUNT(*) - COUNT(${c}) AS null_count`, `COUNT(${c}) AS not_null_count`,
        `COUNT(DISTINCT ${distinctExpr}) AS distinct_count`,
      ];
      if (cat === 'numeric') {
        parts.push(
          `COUNT(*) FILTER (WHERE ${c} = 0) AS zero_count`,
          `COUNT(*) FILTER (WHERE ${c}::text = 'NaN') AS nan_count`,
          `MIN(${c})::text AS min_val`, `MAX(${c})::text AS max_val`,
          `AVG(${c}::double precision)::double precision AS avg_val`,
          `(PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY ${c}::double precision))::text AS median_val`,
        );
      } else if (cat === 'text') {
        parts.push(
          `0 AS zero_count`, `0 AS nan_count`,
          `MIN(${c})::text AS min_val`, `MAX(${c})::text AS max_val`,
          `AVG(LENGTH(${c}))::double precision AS avg_val`,
          `(PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY LENGTH(${c})))::text AS median_val`,
        );
      } else {
        parts.push(
          `0 AS zero_count`, `0 AS nan_count`,
          `MIN(${c}::text) AS min_val`, `MAX(${c}::text) AS max_val`,
          `NULL::double precision AS avg_val`, `NULL::text AS median_val`,
        );
      }
      return `SELECT ${parts.join(', ')} FROM ${tbl}`;
    });
    return selects.join('\nUNION ALL\n');
  },

  bulkHistogramQuery(schema, table, columns) {
    const tbl = qualified(schema, table);
    const wrapped = columns.map(col => {
      const c = q(col.name);
      const alias = col.name.replace(/'/g, "''");
      return `(SELECT '${alias}' AS col_name, value, freq FROM (SELECT ${c}::text AS value, COUNT(*) AS freq FROM ${tbl} WHERE ${c} IS NOT NULL GROUP BY ${c}::text ORDER BY freq DESC LIMIT 20) sub)`;
    });
    return wrapped.join('\nUNION ALL\n');
  },

  bulkUniqueCountQuery(schema, table, columns) {
    const tbl = qualified(schema, table);
    const selects = columns.map(col => {
      const c = q(col.name);
      const alias = col.name.replace(/'/g, "''");
      return `SELECT '${alias}' AS col_name, COUNT(*) AS unique_count FROM (SELECT ${c}::text FROM ${tbl} WHERE ${c} IS NOT NULL GROUP BY ${c}::text HAVING COUNT(*) = 1) sub`;
    });
    return selects.join('\nUNION ALL\n');
  },
};
