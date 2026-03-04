import { invoke } from '@tauri-apps/api/core';
import type {
  StructureTab,
  ErdTab,
  ErdData,
  TriggerInfo,
  ForeignKeyInfo,
  CheckConstraintInfo,
  UniqueConstraintInfo,
  PartitionInfo,
  ColumnProfile,
  ColumnInfo,
  CellValue,
  QueryResult,
} from '$lib/types';
import { decodeMsgpack, generateId, setError } from './shared.svelte';
import { addTab, findTab, setActiveTabId } from './tabs.svelte';
import { getRuntimeId, getSavedConnection, loadColumns, loadIndexes } from './connections.svelte';
import { generateStatsQuery, generateHistogramQuery, generateUniqueCountQuery } from '$lib/utils/profiling-sql';

// ── Structure-specific loaders ──

export async function loadTriggers(savedConnectionId: string, databaseName: string, schema: string, table: string): Promise<TriggerInfo[]> {
  const rid = getRuntimeId(savedConnectionId, databaseName);
  if (!rid) return [];
  try {
    return await invoke('list_triggers', { activeConnectionId: rid, schema, table });
  } catch (e) {
    setError(String(e));
    return [];
  }
}

export async function loadForeignKeys(savedConnectionId: string, databaseName: string, schema: string, table: string): Promise<ForeignKeyInfo[]> {
  const rid = getRuntimeId(savedConnectionId, databaseName);
  if (!rid) return [];
  try {
    return await invoke('list_foreign_keys', { activeConnectionId: rid, schema, table });
  } catch (e) {
    setError(String(e));
    return [];
  }
}

export async function loadCheckConstraints(savedConnectionId: string, databaseName: string, schema: string, table: string): Promise<CheckConstraintInfo[]> {
  const rid = getRuntimeId(savedConnectionId, databaseName);
  if (!rid) return [];
  try {
    return await invoke('list_check_constraints', { activeConnectionId: rid, schema, table });
  } catch (e) {
    setError(String(e));
    return [];
  }
}

export async function loadUniqueConstraints(savedConnectionId: string, databaseName: string, schema: string, table: string): Promise<UniqueConstraintInfo[]> {
  const rid = getRuntimeId(savedConnectionId, databaseName);
  if (!rid) return [];
  try {
    return await invoke('list_unique_constraints', { activeConnectionId: rid, schema, table });
  } catch (e) {
    setError(String(e));
    return [];
  }
}

export async function loadPartitionInfo(savedConnectionId: string, databaseName: string, schema: string, table: string): Promise<PartitionInfo | null> {
  const rid = getRuntimeId(savedConnectionId, databaseName);
  if (!rid) return null;
  try {
    return await invoke('get_partition_info', { activeConnectionId: rid, schema, table });
  } catch (e) {
    setError(String(e));
    return null;
  }
}

export async function getCreateTableSql(savedConnectionId: string, databaseName: string, schema: string, table: string): Promise<string> {
  const rid = getRuntimeId(savedConnectionId, databaseName);
  if (!rid) throw new Error('Not connected');
  return await invoke('get_create_table_sql', { activeConnectionId: rid, schema, table });
}

export async function executeDdl(runtimeConnectionId: string, sql: string): Promise<void> {
  try {
    await invoke('execute_batch', { activeConnectionId: runtimeConnectionId, sql });
  } catch (e) {
    setError(String(e));
    throw e;
  }
}

// ── Structure tab ──

export function openStructureTab(savedConnectionId: string, databaseName: string, schema: string, table: string) {
  const rid = getRuntimeId(savedConnectionId, databaseName);
  if (!rid) return;
  const savedConn = getSavedConnection(savedConnectionId);

  const existing = findTab(
    (t): t is StructureTab => t.type === 'structure' && t.savedConnectionId === savedConnectionId
      && t.schema === schema && t.table === table
  );

  if (existing) {
    setActiveTabId(existing.id);
    return;
  }

  const tab: StructureTab = {
    type: 'structure',
    id: generateId(),
    savedConnectionId,
    runtimeConnectionId: rid,
    connectionName: savedConn?.name ?? 'Unknown',
    databaseName,
    schema,
    table,
    activeSection: 'columns',
    isLoading: true,
    columns: [],
    indexes: [],
    foreignKeys: [],
    checkConstraints: [],
    uniqueConstraints: [],
    triggers: [],
    partitionInfo: null,
    profilingData: null,
    isProfilingLoading: false,
  };

  addTab(tab);
  loadStructureTab(tab.id);
}

export async function loadStructureTab(tabId: string) {
  const tab = findTab((t): t is StructureTab => t.type === 'structure' && t.id === tabId);
  if (!tab) return;

  tab.isLoading = true;

  try {
    const [columns, indexes, foreignKeys, checkConstraints, uniqueConstraints, triggers, partitionInfo] =
      await Promise.all([
        loadColumns(tab.savedConnectionId, tab.databaseName, tab.schema, tab.table),
        loadIndexes(tab.savedConnectionId, tab.databaseName, tab.schema),
        loadForeignKeys(tab.savedConnectionId, tab.databaseName, tab.schema, tab.table),
        loadCheckConstraints(tab.savedConnectionId, tab.databaseName, tab.schema, tab.table),
        loadUniqueConstraints(tab.savedConnectionId, tab.databaseName, tab.schema, tab.table),
        loadTriggers(tab.savedConnectionId, tab.databaseName, tab.schema, tab.table),
        loadPartitionInfo(tab.savedConnectionId, tab.databaseName, tab.schema, tab.table),
      ]);

    const updated = findTab((t): t is StructureTab => t.type === 'structure' && t.id === tabId);
    if (updated) {
      updated.columns = columns;
      updated.indexes = indexes.filter(i => i.table_name === tab.table);
      updated.foreignKeys = foreignKeys;
      updated.checkConstraints = checkConstraints;
      updated.uniqueConstraints = uniqueConstraints;
      updated.triggers = triggers;
      updated.partitionInfo = partitionInfo;
      updated.isLoading = false;
    }
  } catch (e) {
    setError(String(e));
    const updated = findTab((t): t is StructureTab => t.type === 'structure' && t.id === tabId);
    if (updated) {
      updated.isLoading = false;
    }
  }
}

// ── Profiling ──

function extractCellValue(cell: CellValue): string | number | null {
  if (cell === 'Null') return null;
  if (typeof cell === 'object') {
    if ('Int' in cell) return cell.Int;
    if ('Float' in cell) return cell.Float;
    if ('Text' in cell) return cell.Text;
    if ('Bool' in cell) return cell.Bool ? 'true' : 'false';
    if ('Json' in cell) return cell.Json;
    if ('Timestamp' in cell) return cell.Timestamp;
  }
  return null;
}

async function runQuery(runtimeConnectionId: string, sql: string): Promise<QueryResult> {
  const bytes = await invoke('execute_query', { activeConnectionId: runtimeConnectionId, sql });
  return decodeMsgpack<QueryResult>(bytes as ArrayBuffer | number[]);
}

async function profileColumn(runtimeConnectionId: string, schema: string, table: string, col: ColumnInfo): Promise<ColumnProfile> {
  const statsSql = generateStatsQuery(schema, table, col);
  const histSql = generateHistogramQuery(schema, table, col);
  const uniqueSql = generateUniqueCountQuery(schema, table, col);

  const [statsResult, histResult, uniqueResult] = await Promise.all([
    runQuery(runtimeConnectionId, statsSql),
    runQuery(runtimeConnectionId, histSql),
    runQuery(runtimeConnectionId, uniqueSql),
  ]);

  const total = Number(extractCellValue(statsResult.cells[0]) ?? 0);
  const nullCount = Number(extractCellValue(statsResult.cells[1]) ?? 0);
  const notNull = Number(extractCellValue(statsResult.cells[2]) ?? 0);
  const distinct = Number(extractCellValue(statsResult.cells[3]) ?? 0);
  const zeroCount = Number(extractCellValue(statsResult.cells[4]) ?? 0);
  const nanCount = Number(extractCellValue(statsResult.cells[5]) ?? 0);
  const minVal = extractCellValue(statsResult.cells[6]);
  const maxVal = extractCellValue(statsResult.cells[7]);
  const avgVal = extractCellValue(statsResult.cells[8]);
  const medianVal = extractCellValue(statsResult.cells[9]);

  const uniqueCount = Number(extractCellValue(uniqueResult.cells[0]) ?? 0);

  const histogram: { value: string; count: number }[] = [];
  const histCols = histResult.columns.length;
  for (let r = 0; r < histResult.row_count; r++) {
    const val = extractCellValue(histResult.cells[r * histCols]);
    const freq = extractCellValue(histResult.cells[r * histCols + 1]);
    if (val !== null) {
      histogram.push({ value: String(val), count: Number(freq) });
    }
  }

  return {
    column_name: col.name,
    data_type: col.data_type,
    total_count: total,
    null_count: nullCount,
    not_null_count: notNull,
    distinct_count: distinct,
    unique_count: uniqueCount,
    zero_count: zeroCount,
    nan_count: nanCount,
    min: minVal !== null ? String(minVal) : null,
    max: maxVal !== null ? String(maxVal) : null,
    avg: avgVal !== null ? Number(avgVal) : null,
    median: medianVal !== null ? String(medianVal) : null,
    histogram,
  };
}

export async function loadProfilingData(tabId: string) {
  const tab = findTab((t): t is StructureTab => t.type === 'structure' && t.id === tabId);
  if (!tab) return;
  if (tab.columns.length === 0) return;

  tab.isProfilingLoading = true;
  tab.profilingData = null;

  try {
    const profiles = await Promise.all(
      tab.columns.map(col => profileColumn(tab.runtimeConnectionId, tab.schema, tab.table, col))
    );

    const updated = findTab((t): t is StructureTab => t.type === 'structure' && t.id === tabId);
    if (updated) {
      updated.profilingData = profiles;
      updated.isProfilingLoading = false;
    }
  } catch (e) {
    setError(String(e));
    const updated = findTab((t): t is StructureTab => t.type === 'structure' && t.id === tabId);
    if (updated) {
      updated.isProfilingLoading = false;
    }
  }
}

// ── ERD tab ──

export function openErdTab(savedConnectionId: string, databaseName: string, schema: string, focusTable: string | null = null) {
  const rid = getRuntimeId(savedConnectionId, databaseName);
  if (!rid) return;
  const savedConn = getSavedConnection(savedConnectionId);

  const existing = findTab(
    (t): t is ErdTab => t.type === 'erd' && t.savedConnectionId === savedConnectionId
      && t.databaseName === databaseName && t.schema === schema
  );

  if (existing) {
    existing.focusTable = focusTable;
    setActiveTabId(existing.id);
    return;
  }

  const tab: ErdTab = {
    type: 'erd',
    id: generateId(),
    savedConnectionId,
    runtimeConnectionId: rid,
    connectionName: savedConn?.name ?? 'Unknown',
    databaseName,
    schema,
    focusTable,
    tables: [],
    columns: {},
    foreignKeys: {},
    isLoading: true,
  };

  addTab(tab);
  loadErdTab(tab.id);
}

export async function loadErdTab(tabId: string) {
  const tab = findTab((t): t is ErdTab => t.type === 'erd' && t.id === tabId);
  if (!tab) return;

  tab.isLoading = true;

  try {
    const data: ErdData = await invoke('get_erd_data', {
      activeConnectionId: tab.runtimeConnectionId,
      schema: tab.schema,
    });

    const updated = findTab((t): t is ErdTab => t.type === 'erd' && t.id === tabId);
    if (updated) {
      updated.tables = data.tables;
      updated.columns = data.columns;
      updated.foreignKeys = data.foreign_keys;
      updated.isLoading = false;
    }
  } catch (e) {
    setError(String(e));
    const updated = findTab((t): t is ErdTab => t.type === 'erd' && t.id === tabId);
    if (updated) {
      updated.isLoading = false;
    }
  }
}
