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
  CellValue,
  QueryResult,
} from '$lib/types';
import { decodeMsgpack, generateId, setError } from './shared.svelte';
import { addTab, findTab, setActiveTabId } from './tabs.svelte';
import { getRuntimeId, getSavedConnection, loadColumns, loadIndexes } from './connections.svelte';
import { generateBulkStatsQuery, generateBulkHistogramQuery, generateBulkUniqueCountQuery } from '$lib/utils/profiling-sql';

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

export async function loadProfilingData(tabId: string) {
  const tab = findTab((t): t is StructureTab => t.type === 'structure' && t.id === tabId);
  if (!tab) return;
  if (tab.columns.length === 0) return;

  tab.isProfilingLoading = true;
  tab.profilingData = null;

  try {
    const columns = tab.columns;
    const statsSql = generateBulkStatsQuery(tab.schema, tab.table, columns);
    const histSql = generateBulkHistogramQuery(tab.schema, tab.table, columns);
    const uniqueSql = generateBulkUniqueCountQuery(tab.schema, tab.table, columns);

    const [statsResult, histResult, uniqueResult] = await Promise.all([
      runQuery(tab.runtimeConnectionId, statsSql),
      runQuery(tab.runtimeConnectionId, histSql),
      runQuery(tab.runtimeConnectionId, uniqueSql),
    ]);

    // Parse bulk stats: each row = one column (col_name, total, null, not_null, distinct, zero, nan, min, max, avg, median)
    const statsCols = statsResult.columns.length;
    const statsMap = new Map<string, number>();
    for (let r = 0; r < statsResult.row_count; r++) {
      const colName = String(extractCellValue(statsResult.cells[r * statsCols]) ?? '');
      statsMap.set(colName, r);
    }

    // Parse bulk histogram: rows with (col_name, value, freq)
    const histCols = histResult.columns.length;
    const histMap = new Map<string, { value: string; count: number }[]>();
    for (let r = 0; r < histResult.row_count; r++) {
      const base = r * histCols;
      const colName = String(extractCellValue(histResult.cells[base]) ?? '');
      const val = extractCellValue(histResult.cells[base + 1]);
      const freq = extractCellValue(histResult.cells[base + 2]);
      if (val !== null) {
        let arr = histMap.get(colName);
        if (!arr) { arr = []; histMap.set(colName, arr); }
        arr.push({ value: String(val), count: Number(freq) });
      }
    }

    // Parse bulk unique counts: rows with (col_name, unique_count)
    const uniqueCols = uniqueResult.columns.length;
    const uniqueMap = new Map<string, number>();
    for (let r = 0; r < uniqueResult.row_count; r++) {
      const base = r * uniqueCols;
      const colName = String(extractCellValue(uniqueResult.cells[base]) ?? '');
      uniqueMap.set(colName, Number(extractCellValue(uniqueResult.cells[base + 1]) ?? 0));
    }

    // Assemble per-column profiles
    const profiles: ColumnProfile[] = columns.map(col => {
      const rowIdx = statsMap.get(col.name);
      const hasStats = rowIdx !== undefined;
      const base = hasStats ? rowIdx * statsCols : 0;

      const total = hasStats ? Number(extractCellValue(statsResult.cells[base + 1]) ?? 0) : 0;
      const nullCount = hasStats ? Number(extractCellValue(statsResult.cells[base + 2]) ?? 0) : 0;
      const notNull = hasStats ? Number(extractCellValue(statsResult.cells[base + 3]) ?? 0) : 0;
      const distinct = hasStats ? Number(extractCellValue(statsResult.cells[base + 4]) ?? 0) : 0;
      const zeroCount = hasStats ? Number(extractCellValue(statsResult.cells[base + 5]) ?? 0) : 0;
      const nanCount = hasStats ? Number(extractCellValue(statsResult.cells[base + 6]) ?? 0) : 0;
      const minVal = hasStats ? extractCellValue(statsResult.cells[base + 7]) : null;
      const maxVal = hasStats ? extractCellValue(statsResult.cells[base + 8]) : null;
      const avgVal = hasStats ? extractCellValue(statsResult.cells[base + 9]) : null;
      const medianVal = hasStats ? extractCellValue(statsResult.cells[base + 10]) : null;

      return {
        column_name: col.name,
        data_type: col.data_type,
        total_count: total,
        null_count: nullCount,
        not_null_count: notNull,
        distinct_count: distinct,
        unique_count: uniqueMap.get(col.name) ?? 0,
        zero_count: zeroCount,
        nan_count: nanCount,
        min: minVal !== null ? String(minVal) : null,
        max: maxVal !== null ? String(maxVal) : null,
        avg: avgVal !== null ? Number(avgVal) : null,
        median: medianVal !== null ? String(medianVal) : null,
        histogram: histMap.get(col.name) ?? [],
      };
    });

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
