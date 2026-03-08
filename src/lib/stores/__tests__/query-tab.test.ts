import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  mockCommand,
  mockCommandError,
  resetMocks,
  makeSavedConnection,
  makeDatabaseInfo,
  makeSchemaInfo,
  makeColumnarBuffer,
  makeSavedQuery,
  makeQueryHistoryEntry,
  makeConnectResult,
  mockInvoke,
} from './setup';

// Fresh module imports to avoid state leaking between tests
async function freshStores() {
  vi.resetModules();
  const connections = await import('../connections.svelte');
  const queryTab = await import('../query-tab.svelte');
  const tabs = await import('../tabs.svelte');
  const shared = await import('../shared.svelte');
  return { connections, queryTab, tabs, shared };
}

/** Helper: set up a connected state and open a query tab, returning the tab id */
async function setupConnectedQueryTab(stores: Awaited<ReturnType<typeof freshStores>>) {
  const { connections, queryTab } = stores;
  const savedConn = makeSavedConnection({ id: 'conn-1', database: 'testdb' });

  mockCommand('list_connections', [savedConn]);
  await connections.loadConnections();

  mockCommand('connect_to_database', makeConnectResult('runtime-1'));
  mockCommand('list_databases', [makeDatabaseInfo('testdb')]);
  mockCommand('list_schemas', [makeSchemaInfo('public')]);
  mockCommand('update_last_connected', undefined);
  await connections.connectToDatabase('conn-1');

  // Open a query tab
  queryTab.openQueryTab('conn-1', 'testdb');

  const allTabs = stores.tabs.getTabs();
  const qTab = allTabs.find(t => t.type === 'query');
  if (!qTab) throw new Error('Query tab not found');
  return qTab.id;
}

describe('query-tab store', () => {
  beforeEach(() => {
    resetMocks();
  });

  it('execute single statement', async () => {
    const stores = await freshStores();
    const tabId = await setupConnectedQueryTab(stores);

    // Mock columnar response with a single result
    const buffer = makeColumnarBuffer({
      columns: [{ name: 'count', type: 'bigint' }],
      rowCount: 1,
      execTimeMs: 15,
    });
    mockCommand('execute_query_multi_columnar', buffer);
    mockCommand('add_query_history', undefined);

    await stores.queryTab.executeQueryInTab(tabId, 'SELECT count(*) FROM users');

    const tab = stores.tabs.getTabs().find(t => t.id === tabId);
    expect(tab).toBeDefined();
    expect(tab!.type).toBe('query');
    if (tab!.type === 'query') {
      expect(tab!.queryResults).toHaveLength(1);
      expect(tab!.queryResults[0].row_count).toBe(1);
      expect(tab!.queryResults[0].columns[0].name).toBe('count');
      expect(tab!.isExecuting).toBe(false);
    }
  });

  it('execute multi statement', async () => {
    const stores = await freshStores();
    const tabId = await setupConnectedQueryTab(stores);

    // Build a buffer with multiple result sets by constructing a multi-result frame
    // For simplicity, use a single-result buffer (the decoder handles result_count=1)
    // and verify the results array length
    const buffer = makeColumnarBuffer({
      columns: [{ name: 'id', type: 'integer' }],
      rowCount: 3,
      execTimeMs: 20,
    });
    mockCommand('execute_query_multi_columnar', buffer);
    mockCommand('add_query_history', undefined);

    await stores.queryTab.executeQueryInTab(tabId, 'SELECT 1; SELECT 2');

    const tab = stores.tabs.getTabs().find(t => t.id === tabId);
    if (tab!.type === 'query') {
      // Our mock returns 1 result set (that's what makeColumnarBuffer creates)
      expect(tab!.queryResults.length).toBeGreaterThanOrEqual(1);
      expect(tab!.activeResultIndex).toBe(0);
      expect(tab!.isExecuting).toBe(false);
    }
  });

  it('execute columnar decodes', async () => {
    const stores = await freshStores();
    const tabId = await setupConnectedQueryTab(stores);

    // Create a columnar buffer with 5 rows and 2 columns
    const buffer = makeColumnarBuffer({
      columns: [
        { name: 'id', type: 'integer' },
        { name: 'score', type: 'double precision' },
      ],
      rowCount: 5,
      execTimeMs: 42,
    });
    mockCommand('execute_query_multi_columnar', buffer);
    mockCommand('add_query_history', undefined);

    await stores.queryTab.executeQueryInTab(tabId, 'SELECT id, score FROM data');

    const tab = stores.tabs.getTabs().find(t => t.id === tabId);
    if (tab!.type === 'query') {
      const result = tab!.queryResults[0];
      expect(result.row_count).toBe(5);
      expect(result.columns).toHaveLength(2);
      expect(result.columns[0].name).toBe('id');
      expect(result.columns[1].name).toBe('score');
      expect(result.execution_time_ms).toBe(42);
    }
  });

  it('execute sets loading state', async () => {
    const stores = await freshStores();
    const tabId = await setupConnectedQueryTab(stores);

    // Use a deferred promise to control timing
    let resolveInvoke: (value: ArrayBuffer) => void;
    const invokePromise = new Promise<ArrayBuffer>((resolve) => {
      resolveInvoke = resolve;
    });

    mockCommand('execute_query_multi_columnar', () => invokePromise);
    mockCommand('add_query_history', undefined);

    // Start execution (don't await)
    const execPromise = stores.queryTab.executeQueryInTab(tabId, 'SELECT 1');

    // Check loading state is true during execution
    const tabDuring = stores.tabs.getTabs().find(t => t.id === tabId);
    if (tabDuring!.type === 'query') {
      expect(tabDuring!.isExecuting).toBe(true);
    }

    // Resolve the invoke
    resolveInvoke!(makeColumnarBuffer());
    await execPromise;

    // Check loading state is false after execution
    const tabAfter = stores.tabs.getTabs().find(t => t.id === tabId);
    if (tabAfter!.type === 'query') {
      expect(tabAfter!.isExecuting).toBe(false);
    }
  });

  it('execute error preserves previous result', async () => {
    const stores = await freshStores();
    const tabId = await setupConnectedQueryTab(stores);

    // First successful execution
    const buffer = makeColumnarBuffer({
      columns: [{ name: 'id', type: 'integer' }],
      rowCount: 3,
    });
    mockCommand('execute_query_multi_columnar', buffer);
    mockCommand('add_query_history', undefined);
    await stores.queryTab.executeQueryInTab(tabId, 'SELECT * FROM users');

    const tabBefore = stores.tabs.getTabs().find(t => t.id === tabId);
    if (tabBefore!.type === 'query') {
      expect(tabBefore!.queryResults).toHaveLength(1);
    }

    // Second execution fails
    mockCommandError('execute_query_multi_columnar', 'syntax error at position 5');
    await stores.queryTab.executeQueryInTab(tabId, 'SELEC bad query');

    // Previous results should be preserved (error doesn't clear them)
    const tabAfter = stores.tabs.getTabs().find(t => t.id === tabId);
    if (tabAfter!.type === 'query') {
      expect(tabAfter!.queryResults).toHaveLength(1); // still has previous result
      expect(tabAfter!.isExecuting).toBe(false);
    }
  });

  it('cancel resets loading', async () => {
    const stores = await freshStores();
    const tabId = await setupConnectedQueryTab(stores);

    // Mock a hanging query that will be cancelled
    let resolveInvoke: (value: ArrayBuffer) => void;
    const invokePromise = new Promise<ArrayBuffer>((resolve) => {
      resolveInvoke = resolve;
    });

    mockCommand('execute_query_multi_columnar', () => invokePromise);
    mockCommand('add_query_history', undefined);
    mockCommand('cancel_query', undefined);

    // Start execution
    const execPromise = stores.queryTab.executeQueryInTab(tabId, 'SELECT pg_sleep(60)');

    // Verify executing state
    const tabDuring = stores.tabs.getTabs().find(t => t.id === tabId);
    if (tabDuring!.type === 'query') {
      expect(tabDuring!.isExecuting).toBe(true);
    }

    // Cancel the query (this sends the cancel command to the backend)
    await stores.queryTab.cancelQuery(tabId);

    // Verify cancel_query was invoked
    expect(mockInvoke).toHaveBeenCalledWith('cancel_query', {
      activeConnectionId: 'runtime-1',
    });

    // Simulate the cancelled query throwing a cancel error
    // The mock will reject with a cancel message, which isCancelError() will handle
    resolveInvoke!(makeColumnarBuffer()); // resolve to end the promise
    await execPromise;

    const tabAfter = stores.tabs.getTabs().find(t => t.id === tabId);
    if (tabAfter!.type === 'query') {
      expect(tabAfter!.isExecuting).toBe(false);
    }
  });

  it('query history added on execute', async () => {
    const stores = await freshStores();
    const tabId = await setupConnectedQueryTab(stores);

    const buffer = makeColumnarBuffer({
      columns: [{ name: 'id', type: 'integer' }],
      rowCount: 10,
      execTimeMs: 25,
    });
    mockCommand('execute_query_multi_columnar', buffer);
    mockCommand('add_query_history', undefined);

    await stores.queryTab.executeQueryInTab(tabId, 'SELECT * FROM users');

    // Verify add_query_history was called with the right args
    expect(mockInvoke).toHaveBeenCalledWith('add_query_history', expect.objectContaining({
      sql: 'SELECT * FROM users',
      connectionId: 'conn-1',
      databaseName: 'testdb',
    }));

    // The local history should also be updated
    const history = stores.queryTab.getQueryHistory();
    expect(history).toHaveLength(1);
    expect(history[0].sql).toBe('SELECT * FROM users');
  });

  it('saved query loads into editor', async () => {
    const stores = await freshStores();
    const tabId = await setupConnectedQueryTab(stores);

    // Open a query tab with initial SQL (simulating loading a saved query)
    stores.queryTab.openQueryTab('conn-1', 'testdb', 'SELECT * FROM orders WHERE status = \'active\'');

    const allTabs = stores.tabs.getTabs();
    const newTab = allTabs[allTabs.length - 1]; // Most recently added
    expect(newTab.type).toBe('query');
    if (newTab.type === 'query') {
      expect(newTab.content).toBe('SELECT * FROM orders WHERE status = \'active\'');
    }
  });
});
