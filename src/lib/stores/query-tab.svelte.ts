import { invoke } from '@tauri-apps/api/core';
import type {
  QueryTab,
  SavedQuery,
  QueryHistoryEntry,
  CompletionBundle,
  CompletionColumn,
} from '$lib/types';
import { decodeMultiColumnar, generateId, isCancelError, setError } from './shared.svelte';
import { addTab, findTab, setActiveTabId } from './tabs.svelte';
import { getRuntimeId, getSavedConnection, getActiveConnections, connectToSpecificDatabase } from './connections.svelte';

// ── Schema completion cache ──

const schemaCompletionCache = new Map<string, Record<string, string[]>>();

function schemaCompletionKey(connId: string, db: string, schema: string): string {
  return `${connId}:${db}:${schema}`;
}

export function invalidateSchemaCache(connId: string, db: string, schema: string) {
  schemaCompletionCache.delete(schemaCompletionKey(connId, db, schema));
  completionBundleCache.delete(schemaCompletionKey(connId, db, schema));
  // Also clear column caches for this schema
  const prefix = schemaCompletionKey(connId, db, schema) + ':';
  for (const key of completionColumnCache.keys()) {
    if (key.startsWith(prefix)) completionColumnCache.delete(key);
  }
}

export function invalidateSchemaCacheForConnection(connId: string) {
  for (const key of schemaCompletionCache.keys()) {
    if (key.startsWith(connId + ':')) schemaCompletionCache.delete(key);
  }
  for (const key of completionBundleCache.keys()) {
    if (key.startsWith(connId + ':')) completionBundleCache.delete(key);
  }
  for (const key of completionColumnCache.keys()) {
    if (key.startsWith(connId + ':')) completionColumnCache.delete(key);
  }
}

// ── Rich completion caches (hybrid strategy) ──

const completionBundleCache = new Map<string, CompletionBundle>();
const completionColumnCache = new Map<string, CompletionColumn[]>();

function columnCacheKey(connId: string, db: string, schema: string, table: string): string {
  return `${connId}:${db}:${schema}:${table}`;
}

// ── State ──

let savedQueries = $state<SavedQuery[]>([]);
let queryHistory = $state<QueryHistoryEntry[]>([]);
let queryCounter = $state(1);

const TIMEOUT_KEY = 'sakidb:queryTimeoutSeconds';
let queryTimeoutSeconds = $state<number | null>(
  (() => {
    try {
      const stored = localStorage.getItem(TIMEOUT_KEY);
      return stored ? Number(stored) : null;
    } catch { return null; }
  })()
);

// ── Read access ──

export function getSavedQueries(): SavedQuery[] { return savedQueries; }
export function getQueryHistory(): QueryHistoryEntry[] { return queryHistory; }
export function getQueryTimeoutSeconds(): number | null { return queryTimeoutSeconds; }

// ── Open ──

export function openQueryTab(savedConnectionId: string, databaseName: string, initialSql?: string) {
  const rid = getRuntimeId(savedConnectionId, databaseName);
  if (!rid) return;
  const savedConn = getSavedConnection(savedConnectionId);

  const tab: QueryTab = {
    type: 'query',
    id: generateId(),
    savedConnectionId,
    runtimeConnectionId: rid,
    connectionName: savedConn?.name ?? 'Unknown',
    databaseName,
    schemaName: 'public',
    title: `Query ${queryCounter++}`,
    content: initialSql ?? '',
    queryResults: [],
    activeResultIndex: 0,
    isExecuting: false,
    statementTimeoutMs: null,
  };

  addTab(tab);
}

// ── Execute ──

export async function executeQueryInTab(tabId: string, sql: string) {
  const tab = findTab((t): t is QueryTab => t.type === 'query' && t.id === tabId);
  if (!tab) return;

  tab.isExecuting = true;
  tab.content = sql;

  try {
    const bytes = await invoke('execute_query_multi_columnar', {
      activeConnectionId: tab.runtimeConnectionId,
      sql,
    });
    const multi = decodeMultiColumnar(bytes as ArrayBuffer);
    const updated = findTab((t): t is QueryTab => t.type === 'query' && t.id === tabId);
    if (updated) {
      updated.queryResults = multi.results;
      updated.activeResultIndex = 0;
      updated.isExecuting = false;

      const firstResult = multi.results[0];
      const executionTimeMs = multi.total_execution_time_ms ?? null;
      const rowCount = firstResult?.row_count ?? null;

      // Persist to backend (fire-and-forget), prepend locally to avoid full reload
      invoke('add_query_history', {
        sql,
        connectionId: tab.savedConnectionId,
        databaseName: tab.databaseName,
        executionTimeMs,
        rowCount,
      }).catch(() => {});

      const localEntry: QueryHistoryEntry = {
        id: generateId(),
        sql,
        connection_id: tab.savedConnectionId,
        database_name: tab.databaseName,
        executed_at: new Date().toISOString(),
        execution_time_ms: executionTimeMs,
        row_count: rowCount,
      };
      queryHistory = [localEntry, ...queryHistory.slice(0, 99)];
    }
  } catch (e) {
    const msg = String(e);
    if (!isCancelError(msg)) {
      setError(msg);
    }
    const updated = findTab((t): t is QueryTab => t.type === 'query' && t.id === tabId);
    if (updated) {
      updated.isExecuting = false;
    }
  }
}

export function updateQueryTabContent(tabId: string, content: string) {
  const tab = findTab((t): t is QueryTab => t.type === 'query' && t.id === tabId);
  if (!tab) return;
  tab.content = content;
}

export function setActiveResultIndex(tabId: string, index: number) {
  const tab = findTab((t): t is QueryTab => t.type === 'query' && t.id === tabId);
  if (!tab) return;
  tab.activeResultIndex = index;
}

export function updateQueryTabTimeout(tabId: string, timeoutMs: number | null) {
  const tab = findTab((t): t is QueryTab => t.type === 'query' && t.id === tabId);
  if (!tab) return;
  tab.statementTimeoutMs = timeoutMs;
}

export async function cancelQuery(tabId: string) {
  const tab = findTab((t) => t.id === tabId);
  if (!tab || !('runtimeConnectionId' in tab)) return;
  try {
    await invoke('cancel_query', {
      activeConnectionId: tab.runtimeConnectionId,
    });
  } catch (e) {
    setError(String(e));
  }
}

export async function switchQueryTabDatabase(tabId: string, databaseName: string) {
  const tab = findTab((t): t is QueryTab => t.type === 'query' && t.id === tabId);
  if (!tab) return;

  const conn = getActiveConnections().get(tab.savedConnectionId);
  if (!conn) return;

  if (!conn.activeDatabases.has(databaseName)) {
    await connectToSpecificDatabase(tab.savedConnectionId, databaseName);
  }

  const rid = getRuntimeId(tab.savedConnectionId, databaseName);
  if (!rid) return;

  tab.runtimeConnectionId = rid;
  tab.databaseName = databaseName;
  tab.schemaName = 'public';
}

export function switchQueryTabSchema(tabId: string, schemaName: string) {
  const tab = findTab((t): t is QueryTab => t.type === 'query' && t.id === tabId);
  if (!tab) return;
  tab.schemaName = schemaName;
}

export async function getSchemaCompletionData(
  savedConnectionId: string,
  databaseName: string,
  schemaName: string,
): Promise<Record<string, string[]>> {
  const key = schemaCompletionKey(savedConnectionId, databaseName, schemaName);
  const cached = schemaCompletionCache.get(key);
  if (cached) return cached;

  const rid = getRuntimeId(savedConnectionId, databaseName);
  if (!rid) return {};

  try {
    const result: Record<string, string[]> = await invoke('get_schema_completion_data', {
      activeConnectionId: rid,
      schema: schemaName,
    });

    schemaCompletionCache.set(key, result);
    return result;
  } catch (e) {
    setError(String(e));
    return {};
  }
}

// ── Rich completion (hybrid: eager bundle + lazy columns) ──

export async function getCompletionBundle(
  savedConnectionId: string,
  databaseName: string,
  schemaName: string,
): Promise<CompletionBundle | null> {
  const key = schemaCompletionKey(savedConnectionId, databaseName, schemaName);
  const cached = completionBundleCache.get(key);
  if (cached) return cached;

  const rid = getRuntimeId(savedConnectionId, databaseName);
  if (!rid) return null;

  try {
    const result: CompletionBundle = await invoke('get_completion_bundle', {
      activeConnectionId: rid,
      schema: schemaName,
    });
    completionBundleCache.set(key, result);
    return result;
  } catch {
    return null;
  }
}

export async function getTableColumnsForCompletion(
  savedConnectionId: string,
  databaseName: string,
  schemaName: string,
  tableName: string,
): Promise<CompletionColumn[]> {
  const key = columnCacheKey(savedConnectionId, databaseName, schemaName, tableName);
  const cached = completionColumnCache.get(key);
  if (cached) return cached;

  const rid = getRuntimeId(savedConnectionId, databaseName);
  if (!rid) return [];

  try {
    const result: CompletionColumn[] = await invoke('get_table_columns_for_completion', {
      activeConnectionId: rid,
      schema: schemaName,
      table: tableName,
    });
    completionColumnCache.set(key, result);
    return result;
  } catch {
    return [];
  }
}

// ── Query timeout ──

export function setQueryTimeout(seconds: number | null) {
  queryTimeoutSeconds = seconds;
  try {
    if (seconds === null) {
      localStorage.removeItem(TIMEOUT_KEY);
    } else {
      localStorage.setItem(TIMEOUT_KEY, String(seconds));
    }
  } catch { /* localStorage unavailable */ }
}

// ── Saved queries ──

export async function loadSavedQueries() {
  try {
    savedQueries = await invoke('list_saved_queries');
  } catch (e) {
    setError(String(e));
  }
}

export async function loadQueryHistory() {
  try {
    queryHistory = await invoke('list_query_history', { limit: 100 });
  } catch (e) {
    setError(String(e));
  }
}

export async function saveFromHistory(historyId: string, name: string) {
  try {
    await invoke('save_from_history', { historyId, name });
    await loadSavedQueries();
  } catch (e) {
    setError(String(e));
  }
}

export async function updateSavedQuery(id: string, name?: string, sql?: string) {
  try {
    await invoke('update_saved_query', { id, name: name ?? null, sql: sql ?? null });
    await loadSavedQueries();
  } catch (e) {
    setError(String(e));
  }
}

export async function deleteSavedQuery(id: string) {
  try {
    await invoke('delete_saved_query', { id });
    await loadSavedQueries();
  } catch (e) {
    setError(String(e));
  }
}

export async function clearHistory() {
  try {
    await invoke('clear_query_history');
    queryHistory = [];
  } catch (e) {
    setError(String(e));
  }
}
