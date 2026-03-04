function q(name: string): string {
  return `"${name.replace(/"/g, '""')}"`;
}

function qualified(schema: string, name: string): string {
  return `${q(schema)}.${q(name)}`;
}

// ── Columns ──

export function generateAddColumn(
  schema: string,
  table: string,
  col: { name: string; type: string; nullable: boolean; defaultValue?: string },
): string {
  let sql = `ALTER TABLE ${qualified(schema, table)} ADD COLUMN ${q(col.name)} ${col.type}`;
  if (!col.nullable) sql += ' NOT NULL';
  if (col.defaultValue) sql += ` DEFAULT ${col.defaultValue}`;
  return sql + ';';
}

export function generateAlterColumn(
  schema: string,
  table: string,
  colName: string,
  changes: { type?: string; nullable?: boolean; defaultValue?: string | null; rename?: string },
): string {
  const stmts: string[] = [];
  const target = qualified(schema, table);

  if (changes.type) {
    stmts.push(`ALTER TABLE ${target} ALTER COLUMN ${q(colName)} TYPE ${changes.type};`);
  }
  if (changes.nullable === false) {
    stmts.push(`ALTER TABLE ${target} ALTER COLUMN ${q(colName)} SET NOT NULL;`);
  } else if (changes.nullable === true) {
    stmts.push(`ALTER TABLE ${target} ALTER COLUMN ${q(colName)} DROP NOT NULL;`);
  }
  if (changes.defaultValue !== undefined) {
    if (changes.defaultValue === null) {
      stmts.push(`ALTER TABLE ${target} ALTER COLUMN ${q(colName)} DROP DEFAULT;`);
    } else {
      stmts.push(`ALTER TABLE ${target} ALTER COLUMN ${q(colName)} SET DEFAULT ${changes.defaultValue};`);
    }
  }
  if (changes.rename) {
    stmts.push(`ALTER TABLE ${target} RENAME COLUMN ${q(colName)} TO ${q(changes.rename)};`);
  }

  return stmts.join('\n');
}

export function generateDropColumn(schema: string, table: string, colName: string): string {
  return `ALTER TABLE ${qualified(schema, table)} DROP COLUMN ${q(colName)};`;
}

// ── Indexes ──

export function generateCreateIndex(
  schema: string,
  table: string,
  idx: { name: string; columns: string[]; unique: boolean; type: string },
): string {
  const unique = idx.unique ? 'UNIQUE ' : '';
  const cols = idx.columns.map(q).join(', ');
  return `CREATE ${unique}INDEX ${q(idx.name)} ON ${qualified(schema, table)} USING ${idx.type} (${cols});`;
}

export function generateDropIndex(schema: string, indexName: string): string {
  return `DROP INDEX ${qualified(schema, indexName)};`;
}

// ── Foreign Keys ──

export function generateAddForeignKey(
  schema: string,
  table: string,
  fk: {
    name?: string;
    columns: string[];
    refSchema: string;
    refTable: string;
    refColumns: string[];
    onUpdate: string;
    onDelete: string;
  },
): string {
  const constraintName = fk.name || `fk_${table}_${fk.columns.join('_')}`;
  const localCols = fk.columns.map(q).join(', ');
  const foreignCols = fk.refColumns.map(q).join(', ');
  return `ALTER TABLE ${qualified(schema, table)} ADD CONSTRAINT ${q(constraintName)} FOREIGN KEY (${localCols}) REFERENCES ${qualified(fk.refSchema, fk.refTable)} (${foreignCols}) ON UPDATE ${fk.onUpdate} ON DELETE ${fk.onDelete};`;
}

export function generateDropConstraint(schema: string, table: string, constraintName: string): string {
  return `ALTER TABLE ${qualified(schema, table)} DROP CONSTRAINT ${q(constraintName)};`;
}

// ── Triggers ──

export function generateCreateTrigger(
  schema: string,
  table: string,
  trig: {
    name: string;
    timing: string;
    event: string;
    forEach: string;
    functionSchema: string;
    functionName: string;
    condition?: string;
  },
): string {
  let sql = `CREATE TRIGGER ${q(trig.name)} ${trig.timing} ${trig.event} ON ${qualified(schema, table)}\n    FOR EACH ${trig.forEach}`;
  if (trig.condition) {
    sql += `\n    WHEN (${trig.condition})`;
  }
  sql += `\n    EXECUTE FUNCTION ${qualified(trig.functionSchema, trig.functionName)}();`;
  return sql;
}

export function generateDropTrigger(schema: string, table: string, triggerName: string): string {
  return `DROP TRIGGER ${q(triggerName)} ON ${qualified(schema, table)};`;
}

export function generateToggleTrigger(
  schema: string,
  table: string,
  triggerName: string,
  enable: boolean,
): string {
  const action = enable ? 'ENABLE' : 'DISABLE';
  return `ALTER TABLE ${qualified(schema, table)} ${action} TRIGGER ${q(triggerName)};`;
}

// ── Partitions ──

export function generateAddPartition(
  schema: string,
  parentTable: string,
  part: { name: string; forValues: string },
): string {
  return `CREATE TABLE ${qualified(schema, part.name)} PARTITION OF ${qualified(schema, parentTable)} ${part.forValues};`;
}

export function generateDetachPartition(
  schema: string,
  parentTable: string,
  partitionName: string,
): string {
  return `ALTER TABLE ${qualified(schema, parentTable)} DETACH PARTITION ${qualified(schema, partitionName)};`;
}
