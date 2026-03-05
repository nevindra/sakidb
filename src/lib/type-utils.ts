export type PgTypeCategory =
  | 'number'
  | 'boolean'
  | 'text'
  | 'temporal'
  | 'identifier'
  | 'network'
  | 'json'
  | 'binary'
  | 'geometric'
  | 'search'
  | 'xml';

const TYPE_MAP: Record<string, PgTypeCategory> = {
  // Number
  int2: 'number', int4: 'number', int8: 'number',
  smallint: 'number', integer: 'number', bigint: 'number',
  float4: 'number', float8: 'number', real: 'number',
  'double precision': 'number',
  numeric: 'number', decimal: 'number', money: 'number',
  oid: 'number',
  smallserial: 'number', serial: 'number', bigserial: 'number',
  serial2: 'number', serial4: 'number', serial8: 'number',
  // Boolean
  bool: 'boolean', boolean: 'boolean',
  // Temporal
  date: 'temporal',
  time: 'temporal', timetz: 'temporal',
  'time without time zone': 'temporal', 'time with time zone': 'temporal',
  timestamp: 'temporal', timestamptz: 'temporal',
  'timestamp without time zone': 'temporal', 'timestamp with time zone': 'temporal',
  interval: 'temporal',
  // Identifier
  uuid: 'identifier', pg_lsn: 'identifier',
  // Network
  inet: 'network', cidr: 'network', macaddr: 'network', macaddr8: 'network',
  // JSON
  json: 'json', jsonb: 'json',
  // Binary
  bytea: 'binary', bit: 'binary', varbit: 'binary', 'bit varying': 'binary',
  // Geometric
  point: 'geometric', line: 'geometric', lseg: 'geometric', box: 'geometric',
  circle: 'geometric', polygon: 'geometric', path: 'geometric',
  // Search
  tsvector: 'search', tsquery: 'search',
  // XML
  xml: 'xml',
};

export function getTypeCategory(dataType: string): PgTypeCategory {
  const t = dataType.toLowerCase();
  const direct = TYPE_MAP[t];
  if (direct) return direct;
  // Handle parameterized types: numeric(10,2), bit(8), varchar(255), etc.
  const base = t.replace(/\s*\(.*\)$/, '').replace(/\s*\[.*\]$/, '');
  const baseMatch = TYPE_MAP[base];
  if (baseMatch) return baseMatch;
  // Handle aliases
  if (t.startsWith('character varying') || t === 'varchar' || t === 'char' || t === 'bpchar' || t === 'name' || t === 'text') return 'text';
  if (t.startsWith('timestamp')) return 'temporal';
  if (t.startsWith('time')) return 'temporal';
  if (t.startsWith('interval')) return 'temporal';
  if (t.startsWith('bit')) return 'binary';
  if (t.startsWith('numeric') || t.startsWith('decimal')) return 'number';
  return 'text';
}

const CATEGORY_CSS: Record<PgTypeCategory, string> = {
  number: 'text-right tabular-nums',
  boolean: 'text-warning',
  text: '',
  temporal: 'text-success',
  identifier: 'font-mono text-sky-400/80',
  network: 'font-mono text-teal-400/80',
  json: 'font-mono text-primary',
  binary: 'font-mono text-text-dim',
  geometric: 'font-mono text-violet-400/80',
  search: 'text-amber-400/80',
  xml: 'font-mono text-primary',
};

export function getCategoryCss(dataType: string): string {
  return CATEGORY_CSS[getTypeCategory(dataType)];
}

const PLACEHOLDER_MAP: Partial<Record<string, string>> = {
  uuid: 'xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx',
  inet: '192.168.1.0/24',
  cidr: '10.0.0.0/8',
  macaddr: '08:00:2b:01:02:03',
  macaddr8: '08:00:2b:01:02:03:04:05',
  date: 'YYYY-MM-DD',
  time: 'HH:MM:SS',
  timetz: 'HH:MM:SS+00',
  timestamp: 'YYYY-MM-DD HH:MM:SS',
  timestamptz: 'YYYY-MM-DD HH:MM:SS+00',
  interval: '1 year 2 months 3 days',
  point: '(x,y)',
  line: '{A,B,C}',
  lseg: '[(x1,y1),(x2,y2)]',
  box: '(x1,y1),(x2,y2)',
  circle: '<(x,y),r>',
  polygon: '((x1,y1),(x2,y2),...)',
  path: '[(x1,y1),(x2,y2),...]',
  json: '{"key": "value"}',
  jsonb: '{"key": "value"}',
  xml: '<element>...</element>',
  tsvector: "'word1' 'word2'",
  tsquery: "word1 & word2",
  bit: '0101',
  varbit: '0101',
  pg_lsn: '0/0',
};

export function getTypePlaceholder(dataType: string): string {
  const t = dataType.toLowerCase().replace(/\s*\(.*\)$/, '');
  return PLACEHOLDER_MAP[t] ?? '';
}
