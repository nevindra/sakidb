import { invoke } from '@tauri-apps/api/core';
import { SvelteMap } from 'svelte/reactivity';
import { fuzzyMatch, type FuzzyResult } from '$lib/utils/fuzzy';
import type {
  SavedConnection,
  SchemaInfo,
  TableInfo,
  ViewInfo,
  MaterializedViewInfo,
  FunctionInfo,
} from '$lib/types';

// ── Types ──

export type SearchEntryType = 'connection' | 'database' | 'schema' | 'table' | 'view' | 'matview' | 'function';

export type SearchEntry = {
  type: SearchEntryType;
  name: string;
  path: string;
  connectionId: string;
  databaseName?: string;
  schemaName?: string;
};

// ── State ──

let searchIndex = $state<Map<string, SearchEntry>>(new SvelteMap());

const SYSTEM_SCHEMAS = new Set(['pg_catalog', 'information_schema', 'pg_toast']);

// Generation counter per connection — incremented on disconnect to invalidate in-flight prefetches
const connectionGeneration = new Map<string, number>();

// ── Index mutations ──

export function addToIndex(entry: SearchEntry) {
  searchIndex.set(entry.path, entry);
}

export function removeFromIndexByPrefix(prefix: string) {
  for (const key of searchIndex.keys()) {
    if (key.startsWith(prefix)) {
      searchIndex.delete(key);
    }
  }
}

export function invalidateConnection(connectionId: string) {
  connectionGeneration.set(connectionId, (connectionGeneration.get(connectionId) ?? 0) + 1);
}

export function indexConnections(connections: SavedConnection[]) {
  // Collect keys to delete first to avoid mutating during iteration
  const toDelete: string[] = [];
  for (const [key, entry] of searchIndex) {
    if (entry.type === 'connection') toDelete.push(key);
  }
  for (const key of toDelete) searchIndex.delete(key);

  for (const conn of connections) {
    searchIndex.set(conn.id, {
      type: 'connection',
      name: conn.name,
      path: conn.id,
      connectionId: conn.id,
    });
  }
}

async function prefetchSchemaObjects(
  runtimeId: string,
  connectionId: string,
  databaseName: string,
  schemaName: string,
) {
  const gen = connectionGeneration.get(connectionId) ?? 0;
  const prefix = `${connectionId}/${databaseName}/${schemaName}`;

  try {
    const [tables, views, matViews, functions] = await Promise.all([
      invoke('list_tables', { activeConnectionId: runtimeId, schema: schemaName }) as Promise<TableInfo[]>,
      invoke('list_views', { activeConnectionId: runtimeId, schema: schemaName }) as Promise<ViewInfo[]>,
      invoke('list_materialized_views', { activeConnectionId: runtimeId, schema: schemaName }) as Promise<MaterializedViewInfo[]>,
      invoke('list_functions', { activeConnectionId: runtimeId, schema: schemaName }) as Promise<FunctionInfo[]>,
    ]);

    // Bail if the connection was disconnected while we were fetching
    if ((connectionGeneration.get(connectionId) ?? 0) !== gen) return;

    // Batch all entries in a single synchronous block — Svelte 5 coalesces into one update
    for (const t of tables) {
      searchIndex.set(`${prefix}/${t.name}`, { type: 'table', name: t.name, path: `${prefix}/${t.name}`, connectionId, databaseName, schemaName });
    }
    for (const v of views) {
      searchIndex.set(`${prefix}/${v.name}`, { type: 'view', name: v.name, path: `${prefix}/${v.name}`, connectionId, databaseName, schemaName });
    }
    for (const mv of matViews) {
      searchIndex.set(`${prefix}/${mv.name}`, { type: 'matview', name: mv.name, path: `${prefix}/${mv.name}`, connectionId, databaseName, schemaName });
    }
    for (const f of functions) {
      searchIndex.set(`${prefix}/${f.name}`, { type: 'function', name: f.name, path: `${prefix}/${f.name}`, connectionId, databaseName, schemaName });
    }
  } catch {
    // Pre-fetch failure is non-fatal
  }
}

export function indexDatabaseSchemas(
  connectionId: string,
  databaseName: string,
  schemas: SchemaInfo[],
  runtimeId: string,
) {
  const dbPrefix = `${connectionId}/${databaseName}`;
  addToIndex({ type: 'database', name: databaseName, path: dbPrefix, connectionId, databaseName });

  for (const schema of schemas) {
    addToIndex({
      type: 'schema',
      name: schema.name,
      path: `${dbPrefix}/${schema.name}`,
      connectionId,
      databaseName,
      schemaName: schema.name,
    });

    if (!SYSTEM_SCHEMAS.has(schema.name)) {
      prefetchSchemaObjects(runtimeId, connectionId, databaseName, schema.name);
    }
  }
}

// ── Search queries ──

export function getSearchIndex(): Map<string, SearchEntry> { return searchIndex; }

export function searchTree(query: string): Map<string, FuzzyResult> {
  if (!query) return new Map();
  const results = new Map<string, FuzzyResult>();
  for (const [path, entry] of searchIndex) {
    const result = fuzzyMatch(query, entry.name);
    if (result) {
      results.set(path, result);
    }
  }
  return results;
}

export function hasDescendantMatch(prefix: string, matches: Map<string, FuzzyResult>): boolean {
  for (const key of matches.keys()) {
    if (key.startsWith(prefix + '/')) return true;
  }
  return false;
}
