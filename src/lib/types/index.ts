import type { ColumnarResultData } from '$lib/types/query-result-data';

// ── Engine types ──

export type EngineType = 'postgres' | 'sqlite' | 'oracle' | 'redis' | 'mongodb' | 'duckdb' | 'clickhouse';

export interface EngineCapabilities {
  sql: boolean;
  introspection: boolean;
  export: boolean;
  restore: boolean;
  key_value: boolean;
  document: boolean;
  schemas: boolean;
  tables: boolean;
  views: boolean;
  materialized_views: boolean;
  functions: boolean;
  sequences: boolean;
  indexes: boolean;
  triggers: boolean;
  partitions: boolean;
  foreign_tables: boolean;
  explain: boolean;
  multi_database: boolean;
}

export interface ConnectResult {
  runtime_id: string;
  capabilities: EngineCapabilities;
}

export interface OracleDriverStatus {
  found: boolean;
  path: string | null;
  method: string | null;
}

// ── Connection types ──

export interface ConnectionConfig {
  engine: EngineType;
  host: string;
  port: number;
  database: string;
  username: string;
  password: string;
  ssl_mode: string;
  // [Fix: M7] Options field for engine-specific settings like Oracle TNS
  options: Record<string, string>;
}

export interface SavedConnection {
  id: string;
  name: string;
  engine: string;
  host: string;
  port: number;
  database: string;
  username: string;
  ssl_mode: string;
  // [Fix: M7] Options field
  options: Record<string, string>;
  created_at: string;
  updated_at: string;
  last_connected_at: string | null;
}

export interface ConnectionInput {
  name: string;
  engine: string;
  host: string;
  port: number;
  database: string;
  username: string;
  password: string;
  ssl_mode: string;
  // [Fix: M7] Options field for engine-specific settings like Oracle TNS
  options: Record<string, string>;
}

// ── Query result types ──

export interface ColumnDef {
  name: string;
  data_type: string;
}

export type CellValue =
  | 'Null'
  | { Bool: boolean }
  | { Int: number }
  | { Float: number }
  | { Text: string }
  | { Bytes: number[] }
  | { Json: string }
  | { Timestamp: string };

// ── Columnar result types (low-memory representation) ──

export type NumberColumn = { type: 'number'; nulls: Uint8Array; values: Float64Array };
export type BoolColumn   = { type: 'bool';   nulls: Uint8Array; values: Uint8Array };
export type TextColumn   = { type: 'text';   nulls: Uint8Array; values: (string | undefined)[]; _raw: Uint8Array; _offsets: Uint32Array };
export type BytesColumn  = { type: 'bytes';  nulls: Uint8Array; values: Uint8Array[] };
export type ColumnArray  = NumberColumn | BoolColumn | TextColumn | BytesColumn;

export interface QueryResult {
  columns: ColumnDef[];
  cells: CellValue[];       // flat: cells[row * num_cols + col]
  row_count: number;
  execution_time_ms: number;
  truncated: boolean;
}

export interface MultiQueryResult {
  results: QueryResult[];
  total_execution_time_ms: number;
}

export type AnyQueryResult = QueryResult | ColumnarResultData;

export interface PagedResult {
  columns: ColumnDef[];
  cells: CellValue[];       // flat: cells[row * num_cols + col]
  row_count: number;
  page: number;
  page_size: number;
  total_rows_estimate: number | null;
  execution_time_ms: number;
}

// ── Database types ──

export interface DatabaseInfo {
  name: string;
  is_template: boolean;
}

// ── Schema types ──

export interface SchemaInfo {
  name: string;
}

export interface TableInfo {
  name: string;
  row_count_estimate: number | null;
  size_bytes: number | null;
  is_partition: boolean;
  parent_table: string | null;
}

export interface ColumnInfo {
  name: string;
  data_type: string;
  is_nullable: boolean;
  is_primary_key: boolean;
  default_value: string | null;
}

export interface ViewInfo {
  name: string;
  is_updatable: boolean;
}

export interface MaterializedViewInfo {
  name: string;
  row_count_estimate: number | null;
  is_populated: boolean;
}

export interface FunctionInfo {
  name: string;
  kind: string;
  return_type: string;
  argument_types: string;
  language: string;
}

export interface SequenceInfo {
  name: string;
  data_type: string;
  last_value: number | null;
}

export interface IndexInfo {
  name: string;
  table_name: string;
  columns: string;
  is_unique: boolean;
  is_primary: boolean;
  index_type: string;
}

export interface ForeignTableInfo {
  name: string;
  server_name: string;
}

export interface TriggerInfo {
  name: string;
  table_name: string;
  event: string;
  timing: string;
  for_each: string;
  function_name: string;
  function_schema: string;
  condition: string | null;
  is_enabled: boolean;
}

export interface ForeignKeyInfo {
  constraint_name: string;
  columns: string[];
  foreign_table_schema: string;
  foreign_table_name: string;
  foreign_columns: string[];
  on_update: string;
  on_delete: string;
}

export interface CheckConstraintInfo {
  constraint_name: string;
  check_clause: string;
}

export interface UniqueConstraintInfo {
  constraint_name: string;
  columns: string[];
  is_primary: boolean;
}

export interface PartitionInfo {
  strategy: string;
  partition_key: string;
  partitions: PartitionDetail[];
}

export interface PartitionDetail {
  name: string;
  expression: string;
  row_count_estimate: number | null;
}

// ── Completion types (editor autocomplete) ──

export interface CompletionTable {
  name: string;
  kind: string; // "table", "view", "materialized_view"
}

export interface CompletionBundle {
  tables: CompletionTable[];
  functions: FunctionInfo[];
}

export interface CompletionColumn {
  name: string;
  data_type: string;
  is_primary_key: boolean;
  is_nullable: boolean;
}

// ── Query management types ──

export interface SavedQuery {
  id: string;
  name: string;
  sql: string;
  connection_id: string | null;
  database_name: string | null;
  created_at: string;
  updated_at: string;
}

export interface QueryHistoryEntry {
  id: string;
  sql: string;
  connection_id: string | null;
  database_name: string | null;
  executed_at: string;
  execution_time_ms: number | null;
  row_count: number | null;
}

// ── Tab types ──

export interface DataTab {
  type: 'data';
  id: string;
  savedConnectionId: string;
  runtimeConnectionId: string;
  connectionName: string;
  databaseName: string;
  schema: string;
  table: string;
  queryResult: AnyQueryResult | null;
  totalRowEstimate: number;
  isLoading: boolean;
  currentPage: number;
  pageSize: number;
  filters: TableFilter[];
  rawSqlFilter?: string;
}

export type CompareMatchMode = 'position' | 'key';

export interface CompareConfig {
  resultIndexA: number;
  resultIndexB: number;
  matchMode: CompareMatchMode;
  keyColumn?: string;
}

export interface QueryTab {
  type: 'query';
  id: string;
  savedConnectionId: string;
  runtimeConnectionId: string;
  connectionName: string;
  databaseName: string;
  schemaName: string;
  title: string;
  content: string;
  queryResults: (QueryResult | ColumnarResultData)[];
  activeResultIndex: number;
  isExecuting: boolean;
  statementTimeoutMs: number | null;
  compareMode?: boolean;
  compareConfig?: CompareConfig;
}

// ── Data profiling types ──

export interface ColumnProfile {
  column_name: string;
  data_type: string;
  total_count: number;
  null_count: number;
  not_null_count: number;
  distinct_count: number;
  unique_count: number;
  zero_count: number;
  nan_count: number;
  min: string | null;
  max: string | null;
  avg: number | null;
  median: string | null;
  histogram: { value: string; count: number }[];
}

export type StructureSection = 'columns' | 'indexes' | 'relations' | 'triggers' | 'partitions' | 'profiling';

export interface StructureTab {
  type: 'structure';
  id: string;
  savedConnectionId: string;
  runtimeConnectionId: string;
  connectionName: string;
  databaseName: string;
  schema: string;
  table: string;
  activeSection: StructureSection;
  isLoading: boolean;
  columns: ColumnInfo[];
  indexes: IndexInfo[];
  foreignKeys: ForeignKeyInfo[];
  checkConstraints: CheckConstraintInfo[];
  uniqueConstraints: UniqueConstraintInfo[];
  triggers: TriggerInfo[];
  partitionInfo: PartitionInfo | null;
  profilingData: ColumnProfile[] | null;
  isProfilingLoading: boolean;
}

export interface ErdTab {
  type: 'erd';
  id: string;
  savedConnectionId: string;
  runtimeConnectionId: string;
  connectionName: string;
  databaseName: string;
  schema: string;
  focusTable: string | null;
  tables: TableInfo[];
  columns: Record<string, ColumnInfo[]>;
  foreignKeys: Record<string, ForeignKeyInfo[]>;
  isLoading: boolean;
}

export interface ErdData {
  tables: TableInfo[];
  columns: Record<string, ColumnInfo[]>;
  foreign_keys: Record<string, ForeignKeyInfo[]>;
}

export type Tab = DataTab | QueryTab | StructureTab | ErdTab;

// ── Layout types (split pane system) ──

export interface TabGroup {
  type: 'tab-group';
  id: string;
  tabIds: string[];
  activeTabId: string | null;
}

export interface SplitNode {
  type: 'split';
  id: string;
  direction: 'horizontal' | 'vertical';
  ratio: number;
  first: LayoutNode;
  second: LayoutNode;
}

export type LayoutNode = TabGroup | SplitNode;

export interface TableFilter {
  column: string;
  operator: FilterOperator;
  value: string;
}

export type FilterOperator =
  | 'equals'
  | 'not_equals'
  | 'contains'
  | 'starts_with'
  | 'is_null'
  | 'is_not_null'
  | 'gt'
  | 'lt'
  | 'gte'
  | 'lte';

// ── Connection state ──

export interface ActiveDatabase {
  runtimeConnectionId: string;
  schemas: SchemaInfo[];
}

export interface ActiveConnection {
  savedConnectionId: string;
  databases: DatabaseInfo[];
  activeDatabases: Map<string, ActiveDatabase>;
  capabilities: EngineCapabilities;
}

// ── Connection colors ──

export const CONNECTION_COLORS = [
  '#89b4fa', // Blue (default)
  '#a6e3a1', // Green
  '#f38ba8', // Red
  '#f9e2af', // Yellow
  '#cba6f7', // Mauve
  '#fab387', // Peach
  '#74c7ec', // Sapphire
  '#f5c2e7', // Pink
] as const;

export type ConnectionColor = (typeof CONNECTION_COLORS)[number];
