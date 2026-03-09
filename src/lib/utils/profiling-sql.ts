import type { ColumnInfo } from '$lib/types';

function q(name: string): string {
  return `"${name.replace(/"/g, '""')}"`;
}

function qualified(schema: string, name: string): string {
  return schema ? `${q(schema)}.${q(name)}` : q(name);
}

type ColumnCategory = 'numeric' | 'text' | 'bool' | 'other';

function categorize(dataType: string): ColumnCategory {
  const t = dataType.toLowerCase();
  if (/^(smallint|integer|bigint|int[248]?|serial|bigserial|smallserial|numeric|decimal|real|double precision|float[48]?|money)/.test(t)) return 'numeric';
  if (/^(bool|boolean)/.test(t)) return 'bool';
  if (/^(char|varchar|character|text|citext|name|uuid|inet|macaddr|xml)/.test(t)) return 'text';
  return 'other';
}

/** Returns true for types that lack default equality/ordering operators (jsonb, arrays, etc.) */
function needsTextCast(cat: ColumnCategory): boolean {
  return cat === 'other';
}

export function generateStatsQuery(schema: string, table: string, col: ColumnInfo): string {
  const tbl = qualified(schema, table);
  const c = q(col.name);
  const cat = categorize(col.data_type);
  // For types without equality operators, cast to text for DISTINCT
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
      `MIN(${c})::text AS min_val`,
      `MAX(${c})::text AS max_val`,
      `AVG(${c}::double precision)::double precision AS avg_val`,
      `(PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY ${c}::double precision))::text AS median_val`,
    );
  } else if (cat === 'text') {
    parts.push(
      `0 AS zero_count`,
      `0 AS nan_count`,
      `MIN(${c})::text AS min_val`,
      `MAX(${c})::text AS max_val`,
      `AVG(LENGTH(${c}))::double precision AS avg_val`,
      `(PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY LENGTH(${c})))::text AS median_val`,
    );
  } else {
    parts.push(
      `0 AS zero_count`,
      `0 AS nan_count`,
      `MIN(${c}::text) AS min_val`,
      `MAX(${c}::text) AS max_val`,
      `NULL::double precision AS avg_val`,
      `NULL::text AS median_val`,
    );
  }

  return `SELECT ${parts.join(', ')} FROM ${tbl}`;
}

export function generateHistogramQuery(schema: string, table: string, col: ColumnInfo): string {
  const tbl = qualified(schema, table);
  const c = q(col.name);

  return `SELECT ${c}::text AS value, COUNT(*) AS freq FROM ${tbl} WHERE ${c} IS NOT NULL GROUP BY ${c}::text ORDER BY freq DESC LIMIT 20`;
}

export function generateUniqueCountQuery(schema: string, table: string, col: ColumnInfo): string {
  const tbl = qualified(schema, table);
  const c = q(col.name);

  return `SELECT COUNT(*) AS unique_count FROM (SELECT ${c}::text FROM ${tbl} WHERE ${c} IS NOT NULL GROUP BY ${c}::text HAVING COUNT(*) = 1) sub`;
}

// ── Bulk variants (all columns in a single query each) ──

export function generateBulkStatsQuery(schema: string, table: string, columns: ColumnInfo[]): string {
  const tbl = qualified(schema, table);
  const selects = columns.map(col => {
    const c = q(col.name);
    const cat = categorize(col.data_type);
    const distinctExpr = needsTextCast(cat) ? `${c}::text` : c;
    const alias = col.name.replace(/'/g, "''");

    const parts: string[] = [
      `'${alias}' AS col_name`,
      `COUNT(*) AS total_count`,
      `COUNT(*) - COUNT(${c}) AS null_count`,
      `COUNT(${c}) AS not_null_count`,
      `COUNT(DISTINCT ${distinctExpr}) AS distinct_count`,
    ];

    if (cat === 'numeric') {
      parts.push(
        `COUNT(*) FILTER (WHERE ${c} = 0) AS zero_count`,
        `COUNT(*) FILTER (WHERE ${c}::text = 'NaN') AS nan_count`,
        `MIN(${c})::text AS min_val`,
        `MAX(${c})::text AS max_val`,
        `AVG(${c}::double precision)::double precision AS avg_val`,
        `(PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY ${c}::double precision))::text AS median_val`,
      );
    } else if (cat === 'text') {
      parts.push(
        `0 AS zero_count`,
        `0 AS nan_count`,
        `MIN(${c})::text AS min_val`,
        `MAX(${c})::text AS max_val`,
        `AVG(LENGTH(${c}))::double precision AS avg_val`,
        `(PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY LENGTH(${c})))::text AS median_val`,
      );
    } else {
      parts.push(
        `0 AS zero_count`,
        `0 AS nan_count`,
        `MIN(${c}::text) AS min_val`,
        `MAX(${c}::text) AS max_val`,
        `NULL::double precision AS avg_val`,
        `NULL::text AS median_val`,
      );
    }

    return `SELECT ${parts.join(', ')} FROM ${tbl}`;
  });

  return selects.join('\nUNION ALL\n');
}

export function generateBulkHistogramQuery(schema: string, table: string, columns: ColumnInfo[]): string {
  const tbl = qualified(schema, table);
  // Wrap each in a subquery since UNION ALL doesn't allow per-branch ORDER BY/LIMIT
  const wrapped = columns.map(col => {
    const c = q(col.name);
    const alias = col.name.replace(/'/g, "''");
    return `(SELECT '${alias}' AS col_name, value, freq FROM (SELECT ${c}::text AS value, COUNT(*) AS freq FROM ${tbl} WHERE ${c} IS NOT NULL GROUP BY ${c}::text ORDER BY freq DESC LIMIT 20) sub)`;
  });

  return wrapped.join('\nUNION ALL\n');
}

export function generateBulkUniqueCountQuery(schema: string, table: string, columns: ColumnInfo[]): string {
  const tbl = qualified(schema, table);
  const selects = columns.map(col => {
    const c = q(col.name);
    const alias = col.name.replace(/'/g, "''");
    return `SELECT '${alias}' AS col_name, COUNT(*) AS unique_count FROM (SELECT ${c}::text FROM ${tbl} WHERE ${c} IS NOT NULL GROUP BY ${c}::text HAVING COUNT(*) = 1) sub`;
  });

  return selects.join('\nUNION ALL\n');
}
