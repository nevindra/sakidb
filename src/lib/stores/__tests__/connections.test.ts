import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  mockCommand,
  mockCommandError,
  resetMocks,
  makeSavedConnection,
  makeDatabaseInfo,
  makeSchemaInfo,
  makeTableInfo,
  makeConnectResult,
  mockInvoke,
} from './setup';

// Since stores use module-level $state runes, we dynamically import fresh
// modules per test to avoid state leaking between tests.
async function freshStores() {
  vi.resetModules();
  // Re-apply mocks after module reset (setup.ts mocks are registered globally
  // but resetModules clears the cached modules so they re-execute)
  const connections = await import('../connections.svelte');
  const tabs = await import('../tabs.svelte');
  const shared = await import('../shared.svelte');
  return { connections, tabs, shared };
}

describe('connections store', () => {
  beforeEach(() => {
    resetMocks();
  });

  it('connect stores runtime id', async () => {
    const { connections } = await freshStores();
    const savedConn = makeSavedConnection({ id: 'conn-1', database: 'testdb' });

    // Mock list_connections to return our saved connection
    mockCommand('list_connections', [savedConn]);
    await connections.loadConnections();

    // Mock connect flow
    mockCommand('connect_to_database', makeConnectResult('runtime-abc'));
    mockCommand('list_databases', [makeDatabaseInfo('testdb')]);
    mockCommand('list_schemas', [makeSchemaInfo('public')]);
    mockCommand('update_last_connected', undefined);

    await connections.connectToDatabase('conn-1');

    // Verify getRuntimeId returns the connection_id from connect
    const rid = connections.getRuntimeId('conn-1', 'testdb');
    expect(rid).toBe('runtime-abc');
  });

  it('disconnect clears runtime id', async () => {
    const { connections } = await freshStores();
    const savedConn = makeSavedConnection({ id: 'conn-1', database: 'testdb' });

    mockCommand('list_connections', [savedConn]);
    await connections.loadConnections();

    mockCommand('connect_to_database', makeConnectResult('runtime-abc'));
    mockCommand('list_databases', [makeDatabaseInfo('testdb')]);
    mockCommand('list_schemas', [makeSchemaInfo('public')]);
    mockCommand('update_last_connected', undefined);

    await connections.connectToDatabase('conn-1');
    expect(connections.getRuntimeId('conn-1', 'testdb')).toBe('runtime-abc');

    // Disconnect
    mockCommand('disconnect_from_database', undefined);
    await connections.disconnectFromDatabase('conn-1');

    // Runtime id should be gone
    expect(connections.getRuntimeId('conn-1', 'testdb')).toBeNull();
    expect(connections.isConnected('conn-1')).toBe(false);
  });

  it('load schema tree builds hierarchy', async () => {
    const { connections } = await freshStores();
    const savedConn = makeSavedConnection({ id: 'conn-1', database: 'testdb' });

    mockCommand('list_connections', [savedConn]);
    await connections.loadConnections();

    const mockDatabases = [makeDatabaseInfo('testdb'), makeDatabaseInfo('analytics')];
    const mockSchemas = [makeSchemaInfo('public'), makeSchemaInfo('app')];

    mockCommand('connect_to_database', makeConnectResult('runtime-1'));
    mockCommand('list_databases', mockDatabases);
    mockCommand('list_schemas', mockSchemas);
    mockCommand('update_last_connected', undefined);

    await connections.connectToDatabase('conn-1');

    // Verify databases are stored
    const databases = connections.getDatabases('conn-1');
    expect(databases).toHaveLength(2);
    expect(databases.map(d => d.name)).toContain('testdb');
    expect(databases.map(d => d.name)).toContain('analytics');

    // Verify schemas for the connected database
    const schemas = connections.getSchemas('conn-1', 'testdb');
    expect(schemas).toHaveLength(2);
    expect(schemas.map(s => s.name)).toContain('public');
    expect(schemas.map(s => s.name)).toContain('app');

    // Verify tables can be loaded for a schema
    mockCommand('list_tables', [makeTableInfo('users'), makeTableInfo('orders')]);
    const tables = await connections.loadTables('conn-1', 'testdb', 'public');
    expect(tables).toHaveLength(2);
    expect(tables.map(t => t.name)).toContain('users');
  });

  it('load schema tree adapts to capabilities', async () => {
    const { connections } = await freshStores();

    // Postgres-like: nested schemas within databases
    const savedConn = makeSavedConnection({ id: 'conn-pg', database: 'mydb' });
    mockCommand('list_connections', [savedConn]);
    await connections.loadConnections();

    const pgSchemas = [makeSchemaInfo('public'), makeSchemaInfo('auth'), makeSchemaInfo('pg_catalog')];
    mockCommand('connect_to_database', makeConnectResult('runtime-pg'));
    mockCommand('list_databases', [makeDatabaseInfo('mydb')]);
    mockCommand('list_schemas', pgSchemas);
    mockCommand('update_last_connected', undefined);

    await connections.connectToDatabase('conn-pg');

    // Postgres has multiple schemas
    const schemas = connections.getSchemas('conn-pg', 'mydb');
    expect(schemas).toHaveLength(3);
    expect(schemas.map(s => s.name)).toEqual(['public', 'auth', 'pg_catalog']);

    // SQLite-like: flat structure with single schema
    // (disconnect first, then reconnect with different data)
    mockCommand('disconnect_from_database', undefined);
    await connections.disconnectFromDatabase('conn-pg');

    const sqliteConn = makeSavedConnection({ id: 'conn-sqlite', database: 'data.db' });
    mockCommand('list_connections', [sqliteConn]);
    await connections.loadConnections();

    mockCommand('connect_to_database', makeConnectResult('runtime-sqlite'));
    mockCommand('list_databases', [makeDatabaseInfo('main')]);
    mockCommand('list_schemas', [makeSchemaInfo('main')]); // SQLite only has 'main'
    mockCommand('update_last_connected', undefined);

    await connections.connectToDatabase('conn-sqlite');
    const sqliteSchemas = connections.getSchemas('conn-sqlite', 'data.db');
    expect(sqliteSchemas).toHaveLength(1);
    expect(sqliteSchemas[0].name).toBe('main');
  });

  it('reconnect replaces runtime id', async () => {
    const { connections } = await freshStores();
    const savedConn = makeSavedConnection({ id: 'conn-1', database: 'testdb' });

    mockCommand('list_connections', [savedConn]);
    await connections.loadConnections();

    // First connect
    mockCommand('connect_to_database', makeConnectResult('runtime-first'));
    mockCommand('list_databases', [makeDatabaseInfo('testdb')]);
    mockCommand('list_schemas', [makeSchemaInfo('public')]);
    mockCommand('update_last_connected', undefined);
    await connections.connectToDatabase('conn-1');
    expect(connections.getRuntimeId('conn-1', 'testdb')).toBe('runtime-first');

    // Disconnect
    mockCommand('disconnect_from_database', undefined);
    await connections.disconnectFromDatabase('conn-1');
    expect(connections.getRuntimeId('conn-1', 'testdb')).toBeNull();

    // Reconnect with new runtime id
    mockCommand('connect_to_database', makeConnectResult('runtime-second'));
    await connections.connectToDatabase('conn-1');
    expect(connections.getRuntimeId('conn-1', 'testdb')).toBe('runtime-second');
  });

  it('delete connection cleans state', async () => {
    const { connections } = await freshStores();
    const savedConn = makeSavedConnection({ id: 'conn-1', database: 'testdb' });

    mockCommand('list_connections', [savedConn]);
    await connections.loadConnections();
    expect(connections.getSavedConnections()).toHaveLength(1);

    // Connect first
    mockCommand('connect_to_database', makeConnectResult('runtime-1'));
    mockCommand('list_databases', [makeDatabaseInfo('testdb')]);
    mockCommand('list_schemas', [makeSchemaInfo('public')]);
    mockCommand('update_last_connected', undefined);
    await connections.connectToDatabase('conn-1');
    expect(connections.isConnected('conn-1')).toBe(true);

    // Delete should disconnect and remove
    mockCommand('disconnect_from_database', undefined);
    mockCommand('delete_connection', undefined);
    mockCommand('list_connections', []); // After delete, list returns empty
    await connections.deleteConnection('conn-1');

    expect(connections.isConnected('conn-1')).toBe(false);
    expect(connections.getRuntimeId('conn-1', 'testdb')).toBeNull();
    expect(connections.getSavedConnections()).toHaveLength(0);
  });

  it('connect error sets error state', async () => {
    const { connections, shared } = await freshStores();
    const savedConn = makeSavedConnection({ id: 'conn-1', database: 'testdb' });

    mockCommand('list_connections', [savedConn]);
    await connections.loadConnections();

    // Mock connect to throw
    mockCommandError('connect_to_database', 'Connection refused');

    const error = await connections.connectToDatabase('conn-1');
    expect(error).toContain('Connection refused');

    // Error state should be set
    expect(shared.getError()).toContain('Connection refused');

    // Connection should NOT be active
    expect(connections.isConnected('conn-1')).toBe(false);
    expect(connections.isConnecting('conn-1')).toBe(false);
  });
});
