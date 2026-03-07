import { invoke } from '@tauri-apps/api/core';
import { SvelteMap, SvelteSet } from 'svelte/reactivity';
import type {
  SavedConnection,
  ConnectionInput,
  ConnectResult,
  DatabaseInfo,
  SchemaInfo,
  TableInfo,
  ColumnInfo,
  ViewInfo,
  MaterializedViewInfo,
  FunctionInfo,
  SequenceInfo,
  IndexInfo,
  ForeignTableInfo,
  ActiveConnection,
  ActiveDatabase,
  EngineCapabilities,
} from '$lib/types';
import { setError } from './shared.svelte';
import { addToIndex, indexConnections, indexDatabaseSchemas, invalidateConnection, removeFromIndexByPrefix } from './search.svelte';
import { closeTabsByConnection, closeTabsByRuntimeId } from './tabs.svelte';
import { invalidateSchemaCacheForConnection } from './query-tab.svelte';

// ── State ──

let savedConnections = $state<SavedConnection[]>([]);
let activeConnections = $state<Map<string, ActiveConnection>>(new SvelteMap());
let connectingIds = $state<Set<string>>(new SvelteSet());
let editDialogConnectionId = $state<string | null>(null);

// ── Read access ──

export function getSavedConnections(): SavedConnection[] { return savedConnections; }
export function getActiveConnections(): Map<string, ActiveConnection> { return activeConnections; }
export function getConnectingIds(): Set<string> { return connectingIds; }
export function getEditDialogConnectionId(): string | null { return editDialogConnectionId; }
export function hasActiveConnections(): boolean { return activeConnections.size > 0; }

export function getRuntimeId(savedConnectionId: string, databaseName: string): string | null {
  return activeConnections.get(savedConnectionId)?.activeDatabases.get(databaseName)?.runtimeConnectionId ?? null;
}

export function getSavedConnection(id: string): SavedConnection | undefined {
  return savedConnections.find(c => c.id === id);
}

export function getCapabilities(savedConnectionId: string): EngineCapabilities | null {
  return activeConnections.get(savedConnectionId)?.capabilities ?? null;
}

// ── Connection CRUD ──

export async function loadConnections() {
  try {
    savedConnections = await invoke('list_connections');
    indexConnections(savedConnections);
  } catch (e) {
    setError(String(e));
  }
}

export async function saveConnection(input: ConnectionInput) {
  try {
    await invoke('save_connection', { input });
    await loadConnections();
  } catch (e) {
    setError(String(e));
  }
}

export async function deleteConnection(id: string) {
  try {
    if (activeConnections.has(id)) {
      await disconnectFromDatabase(id);
    }
    await invoke('delete_connection', { id });
    await loadConnections();
  } catch (e) {
    setError(String(e));
  }
}

export async function updateConnection(id: string, input: ConnectionInput) {
  try {
    await invoke('update_connection', { id, input });
    await loadConnections();
  } catch (e) {
    setError(String(e));
  }
}

export async function testConnection(input: ConnectionInput, id?: string): Promise<boolean> {
  try {
    await invoke('test_connection', { input, id: id ?? null });
    return true;
  } catch (e) {
    setError(String(e));
    return false;
  }
}

// ── Connect / Disconnect ──

export function isConnected(savedConnectionId: string): boolean {
  return activeConnections.has(savedConnectionId);
}

export function isConnecting(savedConnectionId: string): boolean {
  return connectingIds.has(savedConnectionId);
}

export function getDatabases(savedConnectionId: string): DatabaseInfo[] {
  return activeConnections.get(savedConnectionId)?.databases ?? [];
}

export function getActiveDatabase(savedConnectionId: string, dbName: string): ActiveDatabase | null {
  return activeConnections.get(savedConnectionId)?.activeDatabases.get(dbName) ?? null;
}

export function getRuntimeConnectionId(savedConnectionId: string, dbName?: string): string | null {
  const conn = activeConnections.get(savedConnectionId);
  if (!conn) return null;
  if (dbName) {
    return conn.activeDatabases.get(dbName)?.runtimeConnectionId ?? null;
  }
  const first = conn.activeDatabases.values().next();
  return first.done ? null : first.value.runtimeConnectionId;
}

export function getSchemas(savedConnectionId: string, dbName?: string): SchemaInfo[] {
  const conn = activeConnections.get(savedConnectionId);
  if (!conn) return [];
  if (dbName) {
    return conn.activeDatabases.get(dbName)?.schemas ?? [];
  }
  const first = conn.activeDatabases.values().next();
  return first.done ? [] : first.value.schemas;
}

export function isDatabaseConnected(savedConnectionId: string, dbName: string): boolean {
  return activeConnections.get(savedConnectionId)?.activeDatabases.has(dbName) ?? false;
}

export function isDatabaseConnecting(savedConnectionId: string, dbName: string): boolean {
  return connectingIds.has(`${savedConnectionId}:${dbName}`);
}

export async function connectToDatabase(savedConnectionId: string): Promise<string | null> {
  if (activeConnections.has(savedConnectionId)) return null;
  connectingIds.add(savedConnectionId);
  try {
    const result: ConnectResult = await invoke('connect_to_database', {
      connectionId: savedConnectionId,
    });
    const runtimeId = result.runtime_id;
    const capabilities = result.capabilities;

    const databases: DatabaseInfo[] = capabilities.multi_database
      ? await invoke('list_databases', { activeConnectionId: runtimeId })
      : [];

    const schemas: SchemaInfo[] = capabilities.schemas
      ? await invoke('list_schemas', { activeConnectionId: runtimeId })
      : [];

    const savedConn = savedConnections.find(c => c.id === savedConnectionId);
    const configuredDb = savedConn?.database ?? '';

    const activeDatabases = new SvelteMap<string, ActiveDatabase>();
    activeDatabases.set(configuredDb, {
      runtimeConnectionId: runtimeId,
      schemas,
    });

    activeConnections.set(savedConnectionId, {
      savedConnectionId,
      databases,
      activeDatabases,
      capabilities,
    });

    for (const db of databases) {
      addToIndex({ type: 'database', name: db.name, path: `${savedConnectionId}/${db.name}`, connectionId: savedConnectionId, databaseName: db.name });
    }
    indexDatabaseSchemas(savedConnectionId, configuredDb, schemas, runtimeId);

    await invoke('update_last_connected', { id: savedConnectionId });
    await loadConnections();
    return null;
  } catch (e) {
    setError(String(e));
    return String(e);
  } finally {
    connectingIds.delete(savedConnectionId);
  }
}

export async function connectToSpecificDatabase(savedConnectionId: string, dbName: string) {
  const conn = activeConnections.get(savedConnectionId);
  if (!conn || conn.activeDatabases.has(dbName)) return;

  const connectingKey = `${savedConnectionId}:${dbName}`;
  connectingIds.add(connectingKey);
  try {
    const result: ConnectResult = await invoke('connect_to_database_as', {
      connectionId: savedConnectionId,
      database: dbName,
    });
    const runtimeId = result.runtime_id;
    const schemas: SchemaInfo[] = result.capabilities.schemas
      ? await invoke('list_schemas', { activeConnectionId: runtimeId })
      : [];

    conn.activeDatabases.set(dbName, {
      runtimeConnectionId: runtimeId,
      schemas,
    });

    indexDatabaseSchemas(savedConnectionId, dbName, schemas, runtimeId);
  } catch (e) {
    setError(String(e));
  } finally {
    connectingIds.delete(connectingKey);
  }
}

export async function disconnectFromDatabase(savedConnectionId: string) {
  const conn = activeConnections.get(savedConnectionId);
  if (!conn) return;
  for (const [, db] of conn.activeDatabases) {
    try {
      await invoke('disconnect_from_database', {
        activeConnectionId: db.runtimeConnectionId,
      });
    } catch (e) {
      setError(String(e));
    }
  }
  invalidateConnection(savedConnectionId);
  invalidateSchemaCacheForConnection(savedConnectionId);
  closeTabsByConnection(savedConnectionId);
  activeConnections.delete(savedConnectionId);
  removeFromIndexByPrefix(savedConnectionId + '/');
}

export async function disconnectSpecificDatabase(savedConnectionId: string, dbName: string) {
  const conn = activeConnections.get(savedConnectionId);
  if (!conn) return;
  const db = conn.activeDatabases.get(dbName);
  if (!db) return;
  try {
    await invoke('disconnect_from_database', {
      activeConnectionId: db.runtimeConnectionId,
    });
  } catch (e) {
    setError(String(e));
  }
  invalidateConnection(savedConnectionId);
  closeTabsByRuntimeId(db.runtimeConnectionId);
  conn.activeDatabases.delete(dbName);
  removeFromIndexByPrefix(`${savedConnectionId}/${dbName}/`);
}

// ── Database management ──

export async function dropDatabase(savedConnectionId: string, dbName: string) {
  await disconnectSpecificDatabase(savedConnectionId, dbName);

  try {
    await invoke('drop_database', {
      connectionId: savedConnectionId,
      database: dbName,
    });
  } catch (e) {
    setError(String(e));
    return;
  }

  const conn = activeConnections.get(savedConnectionId);
  if (conn) {
    const anyDb = [...conn.activeDatabases.values()][0];
    if (anyDb) {
      try {
        const databases: DatabaseInfo[] = await invoke('list_databases', {
          activeConnectionId: anyDb.runtimeConnectionId,
        });
        conn.databases = databases;
      } catch (e) {
        setError(String(e));
      }
    }
  }
}

export async function createDatabase(savedConnectionId: string, dbName: string) {
  try {
    await invoke('create_database', {
      connectionId: savedConnectionId,
      database: dbName,
    });
  } catch (e) {
    setError(String(e));
    return;
  }

  await refreshDatabases(savedConnectionId);
}

export async function renameDatabase(savedConnectionId: string, oldName: string, newName: string) {
  await disconnectSpecificDatabase(savedConnectionId, oldName);

  try {
    await invoke('rename_database', {
      connectionId: savedConnectionId,
      oldName,
      newName,
    });
  } catch (e) {
    setError(String(e));
    return;
  }

  await refreshDatabases(savedConnectionId);
}

export async function refreshDatabases(savedConnectionId: string) {
  const conn = activeConnections.get(savedConnectionId);
  if (!conn) return;

  const anyDb = [...conn.activeDatabases.values()][0];
  if (!anyDb) return;

  try {
    const databases: DatabaseInfo[] = await invoke('list_databases', {
      activeConnectionId: anyDb.runtimeConnectionId,
    });
    conn.databases = databases;

    for (const [, dbConn] of conn.activeDatabases) {
      try {
        const schemas: SchemaInfo[] = await invoke('list_schemas', {
          activeConnectionId: dbConn.runtimeConnectionId,
        });
        dbConn.schemas = schemas;
      } catch {
        // Schema refresh failure is non-fatal
      }
    }
  } catch (e) {
    setError(String(e));
  }
}

// ── Schema / Table / Column loading ──

export async function loadTables(savedConnectionId: string, databaseName: string, schema: string): Promise<TableInfo[]> {
  const rid = getRuntimeId(savedConnectionId, databaseName);
  if (!rid) return [];
  try {
    return await invoke('list_tables', { activeConnectionId: rid, schema });
  } catch (e) {
    setError(String(e));
    return [];
  }
}

export async function loadColumns(savedConnectionId: string, databaseName: string, schema: string, table: string): Promise<ColumnInfo[]> {
  const rid = getRuntimeId(savedConnectionId, databaseName);
  if (!rid) return [];
  try {
    return await invoke('list_columns', { activeConnectionId: rid, schema, table });
  } catch (e) {
    setError(String(e));
    return [];
  }
}

export async function loadViews(savedConnectionId: string, databaseName: string, schema: string): Promise<ViewInfo[]> {
  const rid = getRuntimeId(savedConnectionId, databaseName);
  if (!rid) return [];
  try {
    return await invoke('list_views', { activeConnectionId: rid, schema });
  } catch (e) {
    setError(String(e));
    return [];
  }
}

export async function loadMaterializedViews(savedConnectionId: string, databaseName: string, schema: string): Promise<MaterializedViewInfo[]> {
  const rid = getRuntimeId(savedConnectionId, databaseName);
  if (!rid) return [];
  try {
    return await invoke('list_materialized_views', { activeConnectionId: rid, schema });
  } catch (e) {
    setError(String(e));
    return [];
  }
}

export async function loadFunctions(savedConnectionId: string, databaseName: string, schema: string): Promise<FunctionInfo[]> {
  const rid = getRuntimeId(savedConnectionId, databaseName);
  if (!rid) return [];
  try {
    return await invoke('list_functions', { activeConnectionId: rid, schema });
  } catch (e) {
    setError(String(e));
    return [];
  }
}

export async function loadSequences(savedConnectionId: string, databaseName: string, schema: string): Promise<SequenceInfo[]> {
  const rid = getRuntimeId(savedConnectionId, databaseName);
  if (!rid) return [];
  try {
    return await invoke('list_sequences', { activeConnectionId: rid, schema });
  } catch (e) {
    setError(String(e));
    return [];
  }
}

export async function loadIndexes(savedConnectionId: string, databaseName: string, schema: string): Promise<IndexInfo[]> {
  const rid = getRuntimeId(savedConnectionId, databaseName);
  if (!rid) return [];
  try {
    return await invoke('list_indexes', { activeConnectionId: rid, schema });
  } catch (e) {
    setError(String(e));
    return [];
  }
}

export async function loadForeignTables(savedConnectionId: string, databaseName: string, schema: string): Promise<ForeignTableInfo[]> {
  const rid = getRuntimeId(savedConnectionId, databaseName);
  if (!rid) return [];
  try {
    return await invoke('list_foreign_tables', { activeConnectionId: rid, schema });
  } catch (e) {
    setError(String(e));
    return [];
  }
}

// ── UI state ──

export function openEditDialog(connectionId: string) {
  editDialogConnectionId = connectionId;
}

export function closeEditDialog() {
  editDialogConnectionId = null;
}
