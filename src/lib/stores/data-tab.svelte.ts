import { invoke } from '@tauri-apps/api/core';
import type { DataTab, PagedResult, TableFilter } from '$lib/types';
import { QueryResultData } from '$lib/types/query-result-data';
import { decodeMsgpack, generateId, isCancelError, setError } from './shared.svelte';
import { addTab, findTab, setActiveTabId } from './tabs.svelte';
import { getRuntimeId, getSavedConnection } from './connections.svelte';

// ── Open ──

export function openDataTab(savedConnectionId: string, databaseName: string, schema: string, table: string) {
  const rid = getRuntimeId(savedConnectionId, databaseName);
  if (!rid) return;
  const savedConn = getSavedConnection(savedConnectionId);

  const existing = findTab(
    (t): t is DataTab => t.type === 'data' && t.savedConnectionId === savedConnectionId
      && t.schema === schema && t.table === table
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
    const bytes = await invoke('execute_query_paged', {
      activeConnectionId: tab.runtimeConnectionId,
      sql,
      page,
      pageSize: effectivePageSize,
    });
    const result = decodeMsgpack<PagedResult>(bytes as ArrayBuffer | number[]);

    const updated = findTab((t): t is DataTab => t.type === 'data' && t.id === tabId);
    if (updated) {
      updated.queryResult = new QueryResultData(
        result.columns,
        result.cells,
        result.total_rows_estimate ?? result.row_count,
        result.execution_time_ms,
        false,
      );
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
  let sql = `SELECT * FROM "${tab.schema}"."${tab.table}"`;

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
  const col = `"${f.column}"`;
  switch (f.operator) {
    case 'equals': return `${col} = '${f.value.replace(/'/g, "''")}'`;
    case 'not_equals': return `${col} != '${f.value.replace(/'/g, "''")}'`;
    case 'contains': return `${col}::text ILIKE '%${f.value.replace(/'/g, "''")}%'`;
    case 'starts_with': return `${col}::text ILIKE '${f.value.replace(/'/g, "''")}%'`;
    case 'is_null': return `${col} IS NULL`;
    case 'is_not_null': return `${col} IS NOT NULL`;
    case 'gt': return `${col} > '${f.value.replace(/'/g, "''")}'`;
    case 'lt': return `${col} < '${f.value.replace(/'/g, "''")}'`;
    case 'gte': return `${col} >= '${f.value.replace(/'/g, "''")}'`;
    case 'lte': return `${col} <= '${f.value.replace(/'/g, "''")}'`;
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
