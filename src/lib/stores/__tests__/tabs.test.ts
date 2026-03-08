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
} from './setup';
import type { DataTab, QueryTab, Tab } from '$lib/types';

// Fresh module imports to avoid state leaking between tests
async function freshStores() {
  vi.resetModules();
  const connections = await import('../connections.svelte');
  const dataTab = await import('../data-tab.svelte');
  const queryTab = await import('../query-tab.svelte');
  const tabs = await import('../tabs.svelte');
  const shared = await import('../shared.svelte');
  const layout = await import('../layout.svelte');
  return { connections, dataTab, queryTab, tabs, shared, layout };
}

/** Set up a connected state */
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

describe('tabs store', () => {
  beforeEach(() => {
    resetMocks();
  });

  it('open tab adds to list and is active', async () => {
    const stores = await freshStores();
    await setupConnected(stores);

    expect(stores.tabs.getTabs()).toHaveLength(0);

    // Open a query tab
    stores.queryTab.openQueryTab('conn-1', 'testdb');
    expect(stores.tabs.getTabs()).toHaveLength(1);

    const tab = stores.tabs.getTabs()[0];
    expect(tab.type).toBe('query');
    expect(tab.savedConnectionId).toBe('conn-1');
    expect(tab.databaseName).toBe('testdb');

    // The tab should be active (in the active group)
    const activeTab = stores.tabs.getActiveTab();
    expect(activeTab).toBeDefined();
    expect(activeTab!.id).toBe(tab.id);
  });

  it('open duplicate data tab switches to existing', async () => {
    const stores = await freshStores();
    await setupConnected(stores);

    // Mock paged result for data tab loading
    mockCommand('execute_query_paged', encode(makePagedResult()));

    // Open first data tab for users table
    stores.dataTab.openDataTab('conn-1', 'testdb', 'public', 'users');
    expect(stores.tabs.getTabs()).toHaveLength(1);
    const firstTab = stores.tabs.getTabs()[0];

    // Open a query tab to change active tab
    stores.queryTab.openQueryTab('conn-1', 'testdb');
    expect(stores.tabs.getTabs()).toHaveLength(2);
    expect(stores.tabs.getActiveTab()!.type).toBe('query');

    // Open same table again — should switch to existing, not create new
    stores.dataTab.openDataTab('conn-1', 'testdb', 'public', 'users');
    expect(stores.tabs.getTabs()).toHaveLength(2); // Still 2, not 3

    // Active tab should now be the original data tab
    const activeTab = stores.tabs.getActiveTab();
    expect(activeTab).toBeDefined();
    expect(activeTab!.id).toBe(firstTab.id);
    expect(activeTab!.type).toBe('data');
  });

  it('close tab activates neighbor', async () => {
    const stores = await freshStores();
    await setupConnected(stores);

    mockCommand('execute_query_paged', encode(makePagedResult()));

    // Open 3 tabs
    stores.queryTab.openQueryTab('conn-1', 'testdb');
    stores.queryTab.openQueryTab('conn-1', 'testdb');
    stores.dataTab.openDataTab('conn-1', 'testdb', 'public', 'users');

    expect(stores.tabs.getTabs()).toHaveLength(3);

    // The last opened tab should be active
    const tabs = stores.tabs.getTabs();
    const dataTabObj = tabs.find(t => t.type === 'data')!;
    expect(stores.tabs.getActiveTab()!.id).toBe(dataTabObj.id);

    // Close the active tab
    stores.tabs.closeTab(dataTabObj.id);

    expect(stores.tabs.getTabs()).toHaveLength(2);

    // After closing, the adjacent tab (last remaining) should be active
    const activeTab = stores.tabs.getActiveTab();
    expect(activeTab).toBeDefined();
    // Should activate one of the remaining tabs
    expect(stores.tabs.getTabs().some(t => t.id === activeTab!.id)).toBe(true);
  });

  it('close all connection tabs on disconnect', async () => {
    const stores = await freshStores();

    // Set up two connections
    const conn1 = makeSavedConnection({ id: 'conn-1', name: 'Conn 1', database: 'db1' });
    const conn2 = makeSavedConnection({ id: 'conn-2', name: 'Conn 2', database: 'db2' });

    mockCommand('list_connections', [conn1, conn2]);
    await stores.connections.loadConnections();

    // Connect first
    mockCommand('connect_to_database', makeConnectResult('runtime-1'));
    mockCommand('list_databases', [makeDatabaseInfo('db1')]);
    mockCommand('list_schemas', [makeSchemaInfo('public')]);
    mockCommand('update_last_connected', undefined);
    await stores.connections.connectToDatabase('conn-1');

    // Connect second
    mockCommand('connect_to_database', makeConnectResult('runtime-2'));
    mockCommand('list_databases', [makeDatabaseInfo('db2')]);
    mockCommand('list_schemas', [makeSchemaInfo('public')]);
    mockCommand('update_last_connected', undefined);
    await stores.connections.connectToDatabase('conn-2');

    mockCommand('execute_query_paged', encode(makePagedResult()));

    // Open tabs for both connections
    stores.queryTab.openQueryTab('conn-1', 'db1');
    stores.dataTab.openDataTab('conn-1', 'db1', 'public', 'users');
    stores.queryTab.openQueryTab('conn-2', 'db2');

    expect(stores.tabs.getTabs()).toHaveLength(3);

    const conn1Tabs = stores.tabs.getTabs().filter(t => t.savedConnectionId === 'conn-1');
    expect(conn1Tabs).toHaveLength(2);

    // Disconnect conn-1 — should close only conn-1's tabs
    mockCommand('disconnect_from_database', undefined);
    await stores.connections.disconnectFromDatabase('conn-1');

    expect(stores.tabs.getTabs()).toHaveLength(1);
    expect(stores.tabs.getTabs()[0].savedConnectionId).toBe('conn-2');
  });
});
