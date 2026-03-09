import { vi, beforeEach } from 'vitest';
import type {
  SavedConnection,
  DatabaseInfo,
  SchemaInfo,
  TableInfo,
  SavedQuery,
  QueryHistoryEntry,
  PagedResult,
  ConnectResult,
  EngineCapabilities,
} from '$lib/types';
import { encode } from '@msgpack/msgpack';

// ── Mock data factories ──

export function makeCapabilities(overrides: Partial<EngineCapabilities> = {}): EngineCapabilities {
  return {
    sql: true,
    introspection: true,
    export: true,
    restore: true,
    key_value: false,
    document: false,
    schemas: true,
    tables: true,
    views: true,
    materialized_views: true,
    functions: true,
    sequences: true,
    indexes: true,
    triggers: true,
    partitions: true,
    foreign_tables: true,
    explain: true,
    multi_database: true,
    ...overrides,
  };
}

export function makeConnectResult(runtimeId: string, capOverrides: Partial<EngineCapabilities> = {}): ConnectResult {
  return {
    runtime_id: runtimeId,
    capabilities: makeCapabilities(capOverrides),
  };
}

export function makeSavedConnection(overrides: Partial<SavedConnection> = {}): SavedConnection {
  return {
    id: 'conn-1',
    name: 'Test Connection',
    engine: 'postgres',
    host: 'localhost',
    port: 5432,
    database: 'testdb',
    username: 'user',
    ssl_mode: 'prefer',
    created_at: '2025-01-01T00:00:00Z',
    updated_at: '2025-01-01T00:00:00Z',
    last_connected_at: null,
    ...overrides,
  };
}

export function makeDatabaseInfo(name: string): DatabaseInfo {
  return { name, is_template: false };
}

export function makeSchemaInfo(name: string): SchemaInfo {
  return { name };
}

export function makeTableInfo(name: string): TableInfo {
  return {
    name,
    row_count_estimate: 100,
    size_bytes: 8192,
    is_partition: false,
    parent_table: null,
  };
}

export function makeSavedQuery(overrides: Partial<SavedQuery> = {}): SavedQuery {
  return {
    id: 'query-1',
    name: 'Test Query',
    sql: 'SELECT 1',
    connection_id: 'conn-1',
    database_name: 'testdb',
    created_at: '2025-01-01T00:00:00Z',
    updated_at: '2025-01-01T00:00:00Z',
    ...overrides,
  };
}

export function makeQueryHistoryEntry(overrides: Partial<QueryHistoryEntry> = {}): QueryHistoryEntry {
  return {
    id: 'history-1',
    sql: 'SELECT 1',
    connection_id: 'conn-1',
    database_name: 'testdb',
    executed_at: '2025-01-01T00:00:00Z',
    execution_time_ms: 10,
    row_count: 1,
    ...overrides,
  };
}

export function makePagedResult(overrides: Partial<PagedResult> = {}): PagedResult {
  return {
    columns: [{ name: 'id', data_type: 'integer' }, { name: 'name', data_type: 'text' }],
    cells: [{ Int: 1 }, { Text: 'Alice' }],
    row_count: 1,
    page: 0,
    page_size: 50,
    total_rows_estimate: 1,
    execution_time_ms: 5,
    ...overrides,
  };
}

/**
 * Build a minimal columnar binary buffer for testing.
 * Creates a single result with the given columns/rows.
 *
 * Multi-result framing: u32 result_count, u64 total_execution_time_ms,
 * then for each result: u32 byte_length, <encoded ColumnarResult bytes>
 */
export function makeColumnarBuffer(opts: {
  columns?: { name: string; type: string }[];
  rowCount?: number;
  execTimeMs?: number;
} = {}): ArrayBuffer {
  const columns = opts.columns ?? [{ name: 'id', type: 'integer' }];
  const rowCount = opts.rowCount ?? 1;
  const execTimeMs = opts.execTimeMs ?? 5;

  // Build single result payload
  const resultParts: number[] = [];

  // Header (25 bytes)
  // colCount: u32
  pushU32(resultParts, columns.length);
  // rowCount: u64
  pushU64(resultParts, rowCount);
  // execTimeMs: u64
  pushU64(resultParts, execTimeMs);
  // truncated: u8
  resultParts.push(0);
  // padding: u32
  pushU32(resultParts, 0);

  // Column definitions
  const encoder = new TextEncoder();
  for (const col of columns) {
    const nameBytes = encoder.encode(col.name);
    pushU16(resultParts, nameBytes.length);
    resultParts.push(...nameBytes);
    const typeBytes = encoder.encode(col.type);
    pushU16(resultParts, typeBytes.length);
    resultParts.push(...typeBytes);
  }

  // Column data (all number columns with dummy values)
  for (let i = 0; i < columns.length; i++) {
    // type tag: 0 = Number
    resultParts.push(0);
    // nulls array (all non-null)
    for (let r = 0; r < rowCount; r++) resultParts.push(0);

    // Alignment padding for Float64Array (8-byte aligned)
    const currentOffset = resultParts.length;
    // We need to figure out what offset this will be at in the final buffer
    // The multi-result framing adds 12 + 4 bytes before this, so total offset = 16 + currentOffset
    const totalOffset = 16 + currentOffset;
    const padding = (8 - (totalOffset % 8)) % 8;
    for (let p = 0; p < padding; p++) resultParts.push(0);

    // Float64 values
    const floatBuf = new ArrayBuffer(rowCount * 8);
    const floatView = new Float64Array(floatBuf);
    for (let r = 0; r < rowCount; r++) {
      floatView[r] = r + 1;
    }
    const floatBytes = new Uint8Array(floatBuf);
    resultParts.push(...floatBytes);
  }

  const resultPayload = new Uint8Array(resultParts);

  // Build multi-result framing
  const totalSize = 4 + 8 + 4 + resultPayload.length; // result_count + exec_time + byte_length + payload
  const buffer = new ArrayBuffer(totalSize);
  const view = new DataView(buffer);
  const bytes = new Uint8Array(buffer);

  let offset = 0;
  // result_count: u32
  view.setUint32(offset, 1, true); offset += 4;
  // total_execution_time_ms: u64
  view.setBigUint64(offset, BigInt(execTimeMs), true); offset += 8;
  // byte_length: u32
  view.setUint32(offset, resultPayload.length, true); offset += 4;
  // payload
  bytes.set(resultPayload, offset);

  return buffer;
}

/**
 * Build a paged columnar binary buffer for testing.
 * Paging header (21 bytes): u32 page, u32 page_size, u8 has_estimate, i64 estimate, 4 padding.
 * Then: encoded ColumnarResult bytes.
 */
export function makePagedColumnarBuffer(opts: {
  columns?: { name: string; type: string }[];
  rowCount?: number;
  execTimeMs?: number;
  page?: number;
  pageSize?: number;
  totalRowsEstimate?: number | null;
} = {}): ArrayBuffer {
  const columns = opts.columns ?? [{ name: 'id', type: 'integer' }, { name: 'name', type: 'text' }];
  const rowCount = opts.rowCount ?? 1;
  const execTimeMs = opts.execTimeMs ?? 5;
  const page = opts.page ?? 0;
  const pageSize = opts.pageSize ?? 50;
  const estimate = opts.totalRowsEstimate ?? rowCount;

  // Build columnar result payload
  const resultParts: number[] = [];

  // ColumnarResult header (25 bytes)
  pushU32(resultParts, columns.length);
  pushU64(resultParts, rowCount);
  pushU64(resultParts, execTimeMs);
  resultParts.push(0); // truncated
  pushU32(resultParts, 0); // padding

  // Column definitions
  const encoder = new TextEncoder();
  for (const col of columns) {
    const nameBytes = encoder.encode(col.name);
    pushU16(resultParts, nameBytes.length);
    resultParts.push(...nameBytes);
    const typeBytes = encoder.encode(col.type);
    pushU16(resultParts, typeBytes.length);
    resultParts.push(...typeBytes);
  }

  // Column data — all number columns for simplicity
  for (let c = 0; c < columns.length; c++) {
    resultParts.push(0); // type tag: Number
    for (let r = 0; r < rowCount; r++) resultParts.push(0); // nulls

    // Alignment for Float64Array — offset from start of full buffer includes paging header (21 bytes)
    const totalOffset = 21 + resultParts.length;
    const padding = (8 - (totalOffset % 8)) % 8;
    for (let p = 0; p < padding; p++) resultParts.push(0);

    const floatBuf = new ArrayBuffer(rowCount * 8);
    const floatView = new Float64Array(floatBuf);
    for (let r = 0; r < rowCount; r++) floatView[r] = r + 1;
    resultParts.push(...new Uint8Array(floatBuf));
  }

  const resultPayload = new Uint8Array(resultParts);

  // Build full buffer: paging header + columnar payload
  const totalSize = 21 + resultPayload.length;
  const buffer = new ArrayBuffer(totalSize);
  const view = new DataView(buffer);
  const bytes = new Uint8Array(buffer);

  let offset = 0;
  view.setUint32(offset, page, true); offset += 4;
  view.setUint32(offset, pageSize, true); offset += 4;
  if (estimate !== null) {
    bytes[offset] = 1; offset += 1;
    view.setBigInt64(offset, BigInt(estimate), true); offset += 8;
  } else {
    bytes[offset] = 0; offset += 1;
    view.setBigInt64(offset, 0n, true); offset += 8;
  }
  // 4 bytes padding
  offset += 4;

  bytes.set(resultPayload, offset);
  return buffer;
}

function pushU16(arr: number[], val: number) {
  arr.push(val & 0xff, (val >> 8) & 0xff);
}

function pushU32(arr: number[], val: number) {
  arr.push(val & 0xff, (val >> 8) & 0xff, (val >> 16) & 0xff, (val >> 24) & 0xff);
}

function pushU64(arr: number[], val: number) {
  // Write as two u32s (little-endian)
  pushU32(arr, val);
  pushU32(arr, 0);
}

// ── Default mock responses keyed by command name ──

const defaultResponses: Record<string, unknown> = {
  save_connection: 'new-conn-id',
  list_connections: [],
  connect_to_database: makeConnectResult('runtime-1'),
  connect_to_database_as: makeConnectResult('runtime-2'),
  disconnect_from_database: undefined,
  test_connection: undefined,
  update_connection: undefined,
  delete_connection: undefined,
  update_last_connected: undefined,
  list_databases: [makeDatabaseInfo('testdb'), makeDatabaseInfo('other_db')],
  list_schemas: [makeSchemaInfo('public'), makeSchemaInfo('pg_catalog')],
  list_tables: [makeTableInfo('users'), makeTableInfo('orders')],
  list_columns: [],
  list_views: [],
  list_materialized_views: [],
  list_functions: [],
  list_sequences: [],
  list_indexes: [],
  list_foreign_tables: [],
  execute_query_multi_columnar: makeColumnarBuffer(),
  execute_query_paged: encode(makePagedResult()),
  execute_query_paged_columnar: makePagedColumnarBuffer(),
  save_query: undefined,
  list_saved_queries: [],
  update_saved_query: undefined,
  delete_saved_query: undefined,
  add_query_history: undefined,
  list_query_history: [],
  clear_query_history: undefined,
  save_from_history: undefined,
  cancel_query: undefined,
  get_schema_completion_data: {},
  get_completion_bundle: { tables: [], functions: [] },
  get_table_columns_for_completion: [],
  create_database: undefined,
  drop_database: undefined,
  rename_database: undefined,
  export_table_csv: 0,
  export_table_sql: 0,
  restore_from_sql: undefined,
  cancel_restore: undefined,
  cancel_export: undefined,
};

// Per-command overrides that tests can set
let commandOverrides: Map<string, unknown | ((...args: unknown[]) => unknown)> = new Map();

/**
 * Set a mock return value for a specific Tauri command.
 * Pass a function to dynamically compute the return value.
 */
export function mockCommand(command: string, returnValue: unknown | ((...args: unknown[]) => unknown)) {
  commandOverrides.set(command, returnValue);
}

/**
 * Make a specific command throw an error.
 */
export function mockCommandError(command: string, errorMessage: string) {
  commandOverrides.set(command, () => { throw new Error(errorMessage); });
}

/**
 * Clear all command overrides back to defaults.
 */
export function resetMocks() {
  commandOverrides.clear();
  mockInvoke.mockClear();
}

// ── The mock invoke function ──

export const mockInvoke = vi.fn(async (command: string, args?: Record<string, unknown>) => {
  // Check for per-command overrides first
  if (commandOverrides.has(command)) {
    const override = commandOverrides.get(command);
    if (typeof override === 'function') {
      return (override as (args?: Record<string, unknown>) => unknown)(args);
    }
    return override;
  }

  // Fall back to defaults
  if (command in defaultResponses) {
    return defaultResponses[command];
  }

  throw new Error(`Unmocked Tauri command: ${command}`);
});

// ── Mock Tauri modules ──

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...(args as [string, Record<string, unknown>?])),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => {}),
  emit: vi.fn(async () => {}),
  once: vi.fn(async () => () => {}),
}));

vi.mock('@tauri-apps/plugin-updater', () => ({
  check: vi.fn(async () => null),
}));

vi.mock('@tauri-apps/plugin-process', () => ({
  relaunch: vi.fn(async () => {}),
}));

// Mock import.meta.env.DEV
vi.stubGlobal('import', { meta: { env: { DEV: false } } });

// Mock localStorage
const localStorageStore: Record<string, string> = {};
const mockLocalStorage = {
  getItem: vi.fn((key: string) => localStorageStore[key] ?? null),
  setItem: vi.fn((key: string, value: string) => { localStorageStore[key] = value; }),
  removeItem: vi.fn((key: string) => { delete localStorageStore[key]; }),
  clear: vi.fn(() => { Object.keys(localStorageStore).forEach(k => delete localStorageStore[k]); }),
  get length() { return Object.keys(localStorageStore).length; },
  key: vi.fn((index: number) => Object.keys(localStorageStore)[index] ?? null),
};
vi.stubGlobal('localStorage', mockLocalStorage);

// Mock crypto.randomUUID
let uuidCounter = 0;
vi.stubGlobal('crypto', {
  randomUUID: () => `test-uuid-${++uuidCounter}`,
});

// ── Reset state between tests ──

beforeEach(() => {
  resetMocks();
  uuidCounter = 0;
  mockLocalStorage.clear();
});
