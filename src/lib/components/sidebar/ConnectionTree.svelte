<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { EngineType, SavedConnection } from '$lib/types';
  import type { FuzzyResult } from '$lib/utils/fuzzy';
  import { ChevronRight, ChevronDown, Loader2, Server, FolderClosed, FolderOpen } from '@lucide/svelte';
  import { invoke } from '@tauri-apps/api/core';
  import * as ContextMenu from '$lib/components/ui/context-menu';
  import { ContextMenuRenderer, connectionTreeMenuItems } from '$lib/context-menus';
  import type { MenuContext } from '$lib/context-menus';
  import DatabaseNode from './tree/DatabaseNode.svelte';
  import SchemaNode from './tree/SchemaNode.svelte';
  import HighlightMatch from './HighlightMatch.svelte';
  import InputDialog from '$lib/components/ui/input-dialog/InputDialog.svelte';
  import { getDialect } from '$lib/dialects';

  const ENGINE_SHORT: Record<EngineType, string> = {
    postgres: 'PG',
    sqlite: 'SL',
    oracle: 'OR',
    redis: 'RD',
    mongodb: 'MG',
    duckdb: 'DK',
    clickhouse: 'CH',
  };

  let {
    connection,
    filterQuery = '',
    searchResults = new Map(),
  }: {
    connection: SavedConnection;
    filterQuery?: string;
    searchResults?: Map<string, FuzzyResult>;
  } = $props();

  const app = getAppState();

  const isConnected = $derived(app.isConnected(connection.id));
  const isConnecting = $derived(app.isConnecting(connection.id));
  const databases = $derived(app.getDatabases(connection.id));
  const capabilities = $derived(app.getCapabilities(connection.id));
  const schemas = $derived(app.getSchemas(connection.id, connection.database));

  let expanded = $state(false);

  // Auto-expand when connection becomes active (e.g. connected via ConnectionManager)
  $effect(() => {
    if (isConnected) {
      expanded = true;
    }
  });

  const isSearching = $derived(filterQuery.length > 0);
  const selfMatch = $derived(searchResults.get(connection.id));
  const hasChildMatch = $derived(app.hasDescendantMatch(connection.id, searchResults));
  const autoExpand = $derived(isSearching && hasChildMatch);

  // Tab-to-tree sync: expand once when active tab changes to target this connection
  $effect(() => {
    if (app.selectedObjectPath?.startsWith(connection.id + '/')) {
      expanded = true;
    }
  });

  const TEMPLATE_DBS = new Set(['template0', 'template1']);

  const sortedDatabases = $derived(
    [...databases].sort((a, b) => {
      // Configured database first
      if (a.name === connection.database) return -1;
      if (b.name === connection.database) return 1;
      // Templates last
      const aTemplate = a.is_template || TEMPLATE_DBS.has(a.name);
      const bTemplate = b.is_template || TEMPLATE_DBS.has(b.name);
      if (aTemplate !== bTemplate) return aTemplate ? 1 : -1;
      return a.name.localeCompare(b.name);
    })
  );

  const filteredDatabases = $derived(
    isSearching
      ? sortedDatabases.filter(db => {
          const dbPath = `${connection.id}/${db.name}`;
          return searchResults.has(dbPath) || app.hasDescendantMatch(dbPath, searchResults);
        })
      : sortedDatabases
  );

  // For non-multi-db engines: schema expand state
  let expandedSchemas = $state<Set<string>>(new Set());

  $effect(() => {
    if (isConnected && capabilities && !capabilities.multi_database && capabilities.schemas) {
      if (schemas.some(s => s.name === 'public') && !expandedSchemas.has('public')) {
        expandedSchemas.add('public');
        expandedSchemas = new Set(expandedSchemas);
      }
    }
  });

  function toggleSchema(name: string) {
    if (expandedSchemas.has(name)) expandedSchemas.delete(name);
    else expandedSchemas.add(name);
    expandedSchemas = new Set(expandedSchemas);
  }

  async function toggleConnection() {
    if (!isConnected && !isConnecting) {
      expanded = true;
      await app.connectToDatabase(connection.id);
    } else {
      expanded = !expanded;
    }
  }

  const connMenuCtx: MenuContext = $derived({ capabilities, isConnected, engineType: connection.engine });

  let showCreateSchema = $state(false);
  let showCreateDb = $state(false);

  async function handleCreateSchema(name: string) {
    const dialect = getDialect(connection.engine as EngineType);
    const rid = app._getRuntimeId(connection.id, connection.database);
    if (!rid) return;
    const sql = dialect.createSchema(name);
    await invoke('execute_batch', { activeConnectionId: rid, sql });
    await app.refreshDatabases(connection.id);
  }

  async function handleCreateDatabase(name: string) {
    await app.createDatabase(connection.id, name);
  }

  function handleConnMenuAction(id: string) {
    switch (id) {
      case 'new-query': return app.openQueryTab(connection.id, connection.database);
      case 'vacuum': {
        const rid = app._getRuntimeId(connection.id, connection.database);
        if (rid) invoke('vacuum_database', { connId: rid }).catch(e => console.error('VACUUM failed:', e));
        return;
      }
      case 'integrity-check': {
        const rid = app._getRuntimeId(connection.id, connection.database);
        if (rid) invoke('check_integrity', { connId: rid }).then((messages: unknown) => {
          const msgs = messages as string[];
          if (msgs.length === 1 && msgs[0] === 'ok') console.log('Integrity check passed');
          else console.warn('Integrity check issues:', msgs);
        }).catch(e => console.error('Integrity check failed:', e));
        return;
      }
      case 'disconnect': return app.disconnectFromDatabase(connection.id);
      case 'connect': return app.connectToDatabase(connection.id);
      case 'edit': return app.openEditDialog(connection.id);
      case 'delete': return app.deleteConnection(connection.id);
      case 'create-schema': showCreateSchema = true; return;
      case 'create-db': showCreateDb = true; return;
    }
  }
</script>

<ContextMenu.Root>
  <ContextMenu.Trigger class="block w-full">
    <!-- Connection root node -->
    <button
      class="w-full text-left px-2 py-1.5 text-[13px] flex items-center gap-1.5 hover:bg-sidebar-accent transition-colors group"
      onclick={toggleConnection}
    >
      {#if isConnecting}
        <Loader2 class="h-3 w-3 text-muted-foreground animate-spin shrink-0" />
      {:else if (expanded || autoExpand) && isConnected}
        <ChevronDown class="h-3 w-3 text-muted-foreground shrink-0" />
      {:else}
        <ChevronRight class="h-3 w-3 text-muted-foreground shrink-0" />
      {/if}

      <Server class="h-3.5 w-3.5 shrink-0 {isConnected ? 'text-success' : isConnecting ? 'text-warning' : 'text-muted-foreground'}" />

      {#if selfMatch}
        <span class="font-semibold tracking-tight" class:text-foreground={isConnected} class:text-muted-foreground={!isConnected}>
          <HighlightMatch name={connection.name} positions={selfMatch.positions} />
        </span>
      {:else}
        <span
          class="truncate font-semibold tracking-tight"
          class:text-foreground={isConnected}
          class:text-muted-foreground={!isConnected}
        >
          {connection.name}
        </span>
      {/if}

      <span class="ml-auto text-[9px] text-muted-foreground/60 font-mono shrink-0">
        {ENGINE_SHORT[connection.engine as EngineType] ?? ''}
      </span>
    </button>

    <!-- Tree content (when connected + expanded) -->
    {#if isConnected && (expanded || autoExpand)}
      {#if capabilities?.multi_database !== false}
        <!-- Multi-database: Connection > Database > Schema > Objects -->
        {#each filteredDatabases as db (db.name)}
          <DatabaseNode
            database={db}
            connectionId={connection.id}
            isConfiguredDb={db.name === connection.database}
            {filterQuery}
            {searchResults}
          />
        {/each}
      {:else if capabilities?.schemas}
        <!-- Single-database with schemas: Connection > Schema > Objects -->
        {#each schemas as schema (schema.name)}
          <button
            class="w-full text-left pl-6 pr-2 py-0.5 text-xs flex items-center gap-1.5 hover:bg-sidebar-accent transition-colors"
            onclick={() => toggleSchema(schema.name)}
          >
            {#if expandedSchemas.has(schema.name)}
              <ChevronDown class="h-3 w-3 text-muted-foreground shrink-0" />
              <FolderOpen class="h-3 w-3 text-warning shrink-0" />
            {:else}
              <ChevronRight class="h-3 w-3 text-muted-foreground shrink-0" />
              <FolderClosed class="h-3 w-3 text-warning shrink-0" />
            {/if}
            <span class="truncate">{schema.name}</span>
          </button>
          {#if expandedSchemas.has(schema.name)}
            <SchemaNode schemaName={schema.name} connectionId={connection.id} databaseName={connection.database} {filterQuery} {searchResults} />
          {/if}
        {/each}
      {:else}
        <!-- No database, no schemas: Connection > Objects directly -->
        <SchemaNode schemaName="" connectionId={connection.id} databaseName={connection.database} {filterQuery} {searchResults} />
      {/if}
    {/if}
  </ContextMenu.Trigger>

  <ContextMenuRenderer items={connectionTreeMenuItems(connMenuCtx)} ctx={connMenuCtx} onaction={handleConnMenuAction} />
</ContextMenu.Root>

<InputDialog
  bind:open={showCreateSchema}
  title="Create Schema"
  description="Enter a name for the new schema."
  label="Schema name"
  placeholder="new_schema"
  confirmLabel="Create"
  onconfirm={handleCreateSchema}
/>

<InputDialog
  bind:open={showCreateDb}
  title="New Database"
  description="Enter a name for the new database."
  label="Database name"
  placeholder="my_database"
  confirmLabel="Create"
  onconfirm={handleCreateDatabase}
/>
