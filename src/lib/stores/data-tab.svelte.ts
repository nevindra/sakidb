import { invoke } from '@tauri-apps/api/core';
import type { DataTab, TableFilter } from '$lib/types';
import { qualifiedTable, quoteIdent } from '$lib/sql-utils';
import { decodePagedColumnar, generateId, isCancelError, setError } from './shared.svelte';
import { addTab, findTab, setActiveTabId } from './tabs.svelte';
import { getRuntimeId, getSavedConnection } from './connections.svelte';

// ── Open ──

export function openDataTab(savedConnectionId: string, databaseName: string, schema: string, table: string) {
  const rid = getRuntimeId(savedConnectionId, databaseName);
  if (!rid) return;
  const savedConn = getSavedConnection(savedConnectionId);

  const existing = findTab(
    (t): t is DataTab => t.type === 'data' && t.savedConnectionId === savedConnectionId
      && t.databaseName === databaseName && t.schema === schema && t.table === table
  );

  if (existing) {
    setActiveTabId(existing.id);
    return;
  }

  const tab: DataTab = {
    type: 'data',
    id: generateId(),
    savedConnectionId,
    runtimeConnectionId: rid,
    connectionName: savedConn?.name ?? 'Unknown',
    databaseName,
    schema,
    table,
    queryResult: null,
    totalRowEstimate: 0,
    isLoading: true,
    currentPage: 0,
    pageSize: 50,
    filters: [],
  };

  addTab(tab);
  loadDataTab(tab.id);
}

// ── Load / Page ──

export async function loadDataTab(tabId: string, page: number = 0, pageSize?: number) {
  const tab = findTab((t): t is DataTab => t.type === 'data' && t.id === tabId);
  if (!tab) return;

  const effectivePageSize = pageSize ?? tab.pageSize;

  tab.isLoading = true;

  try {
    const sql = buildDataTabQuery(tab);
    const bytes = await invoke('execute_query_paged_columnar', {
      activeConnectionId: tab.runtimeConnectionId,
      sql,
      page,
      pageSize: effectivePageSize,
    });
    const paged = decodePagedColumnar(bytes as ArrayBuffer);

    const updated = findTab((t): t is DataTab => t.type === 'data' && t.id === tabId);
    if (updated) {
      updated.queryResult = paged.result;
      updated.totalRowEstimate = paged.total_rows_estimate ?? paged.result.row_count;
      updated.currentPage = page;
      updated.isLoading = false;
    }
  } catch (e) {
    const msg = String(e);
    if (!isCancelError(msg)) {
      setError(msg);
    }
    const updated = findTab((t): t is DataTab => t.type === 'data' && t.id === tabId);
    if (updated) {
      updated.isLoading = false;
    }
  }
}

// ── Query building ──

export function buildDataTabQuery(tab: DataTab): string {
  let sql = `SELECT * FROM ${qualifiedTable(tab.schema, tab.table)}`;

  const whereClauses: string[] = [];

  if (tab.filters.length > 0) {
    const filterClauses = tab.filters.map(f => filterToSql(f)).filter(Boolean);
    whereClauses.push(...filterClauses);
  }

  if (whereClauses.length > 0) {
    sql += ` WHERE ${whereClauses.join(' AND ')}`;
  }

  return sql;
}

export function filterToSql(f: TableFilter): string {
  const col = quoteIdent(f.column);
  const escaped = f.value.replace(/'/g, "''");
  switch (f.operator) {
    case 'equals': return `${col} = '${escaped}'`;
    case 'not_equals': return `${col} != '${escaped}'`;
    case 'contains': return `${col}::text ILIKE '%${escaped}%'`;
    case 'starts_with': return `${col}::text ILIKE '${escaped}%'`;
    case 'is_null': return `${col} IS NULL`;
    case 'is_not_null': return `${col} IS NOT NULL`;
    case 'gt': return `${col} > '${escaped}'`;
    case 'lt': return `${col} < '${escaped}'`;
    case 'gte': return `${col} >= '${escaped}'`;
    case 'lte': return `${col} <= '${escaped}'`;
    default: return '';
  }
}

// ── Filter / Page-size updates ──

export function updateDataTabFilters(tabId: string, filters: TableFilter[]) {
  const tab = findTab((t): t is DataTab => t.type === 'data' && t.id === tabId);
  if (!tab) return;
  tab.filters = filters;
  loadDataTab(tabId);
}

export function updateDataTabPageSize(tabId: string, pageSize: number) {
  const tab = findTab((t): t is DataTab => t.type === 'data' && t.id === tabId);
  if (!tab) return;
  tab.pageSize = pageSize;
  loadDataTab(tabId, 0, pageSize);
}
