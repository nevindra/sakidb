import { describe, it, expect, vi, beforeEach } from 'vitest';
import { encode } from '@msgpack/msgpack';
import {
  mockCommand,
  resetMocks,
  makeSavedConnection,
  makeDatabaseInfo,
  makeSchemaInfo,
  makePagedResult,
  makeConnectResult,
  mockInvoke,
} from './setup';
import type { PagedResult, DataTab } from '$lib/types';

// Fresh module imports to avoid state leaking between tests
async function freshStores() {
  vi.resetModules();
  const connections = await import('../connections.svelte');
  const dataTab = await import('../data-tab.svelte');
  const tabs = await import('../tabs.svelte');
  const shared = await import('../shared.svelte');
  return { connections, dataTab, tabs, shared };
}

/** Set up a connected state for testing data tabs */
async function setupConnected(stores: Awaited<ReturnType<typeof freshStores>>) {
  const { connections } = stores;
  const savedConn = makeSavedConnection({ id: 'conn-1', database: 'testdb' });

  mockCommand('list_connections', [savedConn]);
  await connections.loadConnections();

  mockCommand('connect_to_database', makeConnectResult('runtime-1'));
  mockCommand('list_databases', [makeDatabaseInfo('testdb')]);
  mockCommand('list_schemas', [makeSchemaInfo('public')]);
  mockCommand('update_last_connected', undefined);
  await connections.connectToDatabase('conn-1');
}

function makePagedMsgpack(overrides: Partial<PagedResult> = {}): Uint8Array {
  return encode(makePagedResult(overrides));
}

describe('data-tab store', () => {
  beforeEach(() => {
    resetMocks();
  });

  it('load page sets rows and total', async () => {
    const stores = await freshStores();
    await setupConnected(stores);

    const pagedResult = makePagedResult({
      columns: [
        { name: 'id', data_type: 'integer' },
        { name: 'name', data_type: 'text' },
      ],
      cells: [{ Int: 1 }, { Text: 'Alice' }, { Int: 2 }, { Text: 'Bob' }],
      row_count: 2,
      total_rows_estimate: 100,
      execution_time_ms: 10,
    });
    mockCommand('execute_query_paged', encode(pagedResult));

    stores.dataTab.openDataTab('conn-1', 'testdb', 'public', 'users');

    // Wait for the initial loadDataTab to complete
    await vi.waitFor(() => {
      const tab = stores.tabs.getTabs().find(t => t.type === 'data') as DataTab | undefined;
      expect(tab).toBeDefined();
      expect(tab!.isLoading).toBe(false);
    });

    const tab = stores.tabs.getTabs().find(t => t.type === 'data') as DataTab;
    expect(tab.queryResult).not.toBeNull();
    // QueryResultData wraps the paged result
    expect(tab.queryResult!.columns).toHaveLength(2);
    expect(tab.queryResult!.row_count).toBe(100); // total_rows_estimate
    expect(tab.currentPage).toBe(0);
  });

  it('next page increments offset', async () => {
    const stores = await freshStores();
    await setupConnected(stores);

    // Initial load
    const page0 = makePagedResult({ page: 0, total_rows_estimate: 200 });
    mockCommand('execute_query_paged', encode(page0));
    stores.dataTab.openDataTab('conn-1', 'testdb', 'public', 'users');

    await vi.waitFor(() => {
      const tab = stores.tabs.getTabs().find(t => t.type === 'data') as DataTab | undefined;
      expect(tab?.isLoading).toBe(false);
    });

    const tab = stores.tabs.getTabs().find(t => t.type === 'data') as DataTab;
    expect(tab.currentPage).toBe(0);

    // Load next page
    const page1 = makePagedResult({ page: 1, total_rows_estimate: 200 });
    mockCommand('execute_query_paged', encode(page1));
    await stores.dataTab.loadDataTab(tab.id, 1);

    await vi.waitFor(() => {
      const updatedTab = stores.tabs.getTabs().find(t => t.id === tab.id) as DataTab;
      expect(updatedTab.currentPage).toBe(1);
    });

    // Verify the invoke was called with page=1
    expect(mockInvoke).toHaveBeenCalledWith('execute_query_paged', expect.objectContaining({
      page: 1,
    }));
  });

  it('apply filter resets to page one', async () => {
    const stores = await freshStores();
    await setupConnected(stores);

    // Initial load at page 3
    const page3 = makePagedResult({ page: 0, total_rows_estimate: 500 });
    mockCommand('execute_query_paged', encode(page3));
    stores.dataTab.openDataTab('conn-1', 'testdb', 'public', 'orders');

    await vi.waitFor(() => {
      const tab = stores.tabs.getTabs().find(t => t.type === 'data') as DataTab | undefined;
      expect(tab?.isLoading).toBe(false);
    });

    const tab = stores.tabs.getTabs().find(t => t.type === 'data') as DataTab;

    // Navigate to page 3 first
    mockCommand('execute_query_paged', encode(makePagedResult({ page: 3 })));
    await stores.dataTab.loadDataTab(tab.id, 3);

    await vi.waitFor(() => {
      const t = stores.tabs.getTabs().find(t => t.id === tab.id) as DataTab;
      expect(t.currentPage).toBe(3);
    });

    // Apply filter — should reset to page 0
    const filtered = makePagedResult({ page: 0, total_rows_estimate: 10 });
    mockCommand('execute_query_paged', encode(filtered));

    stores.dataTab.updateDataTabFilters(tab.id, [
      { column: 'status', operator: 'equals', value: 'active' },
    ]);

    await vi.waitFor(() => {
      const t = stores.tabs.getTabs().find(t => t.id === tab.id) as DataTab;
      expect(t.currentPage).toBe(0);
    });

    // Verify the filter was applied and query was rebuilt
    const t = stores.tabs.getTabs().find(t => t.id === tab.id) as DataTab;
    expect(t.filters).toHaveLength(1);
    expect(t.filters[0].column).toBe('status');
  });

  it('buildDataTabQuery applies filters correctly', async () => {
    const stores = await freshStores();

    // Test the pure query building function
    const mockTab: DataTab = {
      type: 'data',
      id: 'test',
      savedConnectionId: 'conn-1',
      runtimeConnectionId: 'runtime-1',
      connectionName: 'Test',
      databaseName: 'testdb',
      schema: 'public',
      table: 'users',
      queryResult: null,
      isLoading: false,
      currentPage: 0,
      pageSize: 50,
      filters: [
        { column: 'age', operator: 'gt', value: '18' },
        { column: 'status', operator: 'equals', value: 'active' },
      ],
    };

    const sql = stores.dataTab.buildDataTabQuery(mockTab);
    expect(sql).toContain('SELECT * FROM "public"."users"');
    expect(sql).toContain('"age" > \'18\'');
    expect(sql).toContain('"status" = \'active\'');
    expect(sql).toContain('WHERE');
    expect(sql).toContain('AND');
  });

  it('filterToSql handles operators', async () => {
    const stores = await freshStores();

    expect(stores.dataTab.filterToSql({ column: 'name', operator: 'contains', value: 'test' }))
      .toContain("ILIKE '%test%'");

    expect(stores.dataTab.filterToSql({ column: 'id', operator: 'is_null', value: '' }))
      .toBe('"id" IS NULL');

    expect(stores.dataTab.filterToSql({ column: 'id', operator: 'is_not_null', value: '' }))
      .toBe('"id" IS NOT NULL');

    expect(stores.dataTab.filterToSql({ column: 'name', operator: 'starts_with', value: 'A' }))
      .toContain("ILIKE 'A%'");

    expect(stores.dataTab.filterToSql({ column: 'price', operator: 'gte', value: '100' }))
      .toBe("\"price\" >= '100'");

    expect(stores.dataTab.filterToSql({ column: 'price', operator: 'lte', value: '500' }))
      .toBe("\"price\" <= '500'");
  });

  it('update page size reloads from page 0', async () => {
    const stores = await freshStores();
    await setupConnected(stores);

    mockCommand('execute_query_paged', encode(makePagedResult()));
    stores.dataTab.openDataTab('conn-1', 'testdb', 'public', 'users');

    await vi.waitFor(() => {
      const tab = stores.tabs.getTabs().find(t => t.type === 'data') as DataTab | undefined;
      expect(tab?.isLoading).toBe(false);
    });

    const tab = stores.tabs.getTabs().find(t => t.type === 'data') as DataTab;

    // Update page size
    mockCommand('execute_query_paged', encode(makePagedResult({ page_size: 100 })));
    stores.dataTab.updateDataTabPageSize(tab.id, 100);

    await vi.waitFor(() => {
      const t = stores.tabs.getTabs().find(t => t.id === tab.id) as DataTab;
      expect(t.isLoading).toBe(false);
    });

    const updated = stores.tabs.getTabs().find(t => t.id === tab.id) as DataTab;
    expect(updated.pageSize).toBe(100);
    expect(updated.currentPage).toBe(0);

    // Verify invoke was called with pageSize=100
    expect(mockInvoke).toHaveBeenCalledWith('execute_query_paged', expect.objectContaining({
      pageSize: 100,
    }));
  });
});
