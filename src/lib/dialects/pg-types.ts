/** Common PostgreSQL data types grouped by category, for the column type combobox. */
export const PG_TYPE_GROUPS: { label: string; types: string[] }[] = [
  {
    label: 'Numeric',
    types: ['smallint', 'integer', 'bigint', 'serial', 'bigserial', 'numeric', 'decimal', 'real', 'double precision', 'money'],
  },
  {
    label: 'Text',
    types: ['text', 'varchar', 'char', 'character varying', 'citext', 'name'],
  },
  {
    label: 'Boolean',
    types: ['boolean'],
  },
  {
    label: 'Date / Time',
    types: ['date', 'time', 'time with time zone', 'timestamp', 'timestamp with time zone', 'interval'],
  },
  {
    label: 'JSON',
    types: ['json', 'jsonb'],
  },
  {
    label: 'UUID',
    types: ['uuid'],
  },
  {
    label: 'Binary',
    types: ['bytea'],
  },
  {
    label: 'Network',
    types: ['inet', 'cidr', 'macaddr'],
  },
  {
    label: 'Geometric',
    types: ['point', 'line', 'lseg', 'box', 'circle', 'polygon', 'path'],
  },
  {
    label: 'Other',
    types: ['xml', 'tsvector', 'tsquery', 'bit', 'varbit', 'pg_lsn', 'oid'],
  },
];

/** Flat list of all common PG types for filtering. */
export const PG_TYPES: string[] = PG_TYPE_GROUPS.flatMap(g => g.types);

/** Types that accept a precision/length parameter, with placeholder hints. */
export const PG_PRECISION_TYPES: Record<string, string> = {
  'varchar': '255',
  'character varying': '255',
  'char': '1',
  'numeric': '10,2',
  'decimal': '10,2',
  'bit': '1',
  'varbit': '64',
  'time': '6',
  'time with time zone': '6',
  'timestamp': '6',
  'timestamp with time zone': '6',
  'interval': '',
};
