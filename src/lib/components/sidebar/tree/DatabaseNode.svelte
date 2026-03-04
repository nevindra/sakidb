<script lang="ts">
  import { getAppState } from '$lib/stores';
  import { untrack } from 'svelte';
  import type { DatabaseInfo } from '$lib/types';
  import type { FuzzyResult } from '$lib/utils/fuzzy';
  import { ChevronRight, ChevronDown, Database, Loader2, FolderClosed, FolderOpen } from '@lucide/svelte';
  import * as ContextMenu from '$lib/components/ui/context-menu';
  import ConfirmDialog from '$lib/components/ui/confirm-dialog/ConfirmDialog.svelte';
  import InputDialog from '$lib/components/ui/input-dialog/InputDialog.svelte';
  import SchemaNode from './SchemaNode.svelte';
  import RestoreDialog from './RestoreDialog.svelte';
  import HighlightMatch from '../HighlightMatch.svelte';

  let {
    database,
    connectionId,
    isConfiguredDb = false,
    filterQuery = '',
    searchResults = new Map(),
  }: {
    database: DatabaseInfo;
    connectionId: string;
    isConfiguredDb?: boolean;
    filterQuery?: string;
    searchResults?: Map<string, FuzzyResult>;
  } = $props();

  const app = getAppState();

  const SYSTEM_SCHEMAS = new Set(['pg_catalog', 'information_schema', 'pg_toast']);

  const isDbConnected = $derived(app.isDatabaseConnected(connectionId, database.name));
  const isDbConnecting = $derived(app.isDatabaseConnecting(connectionId, database.name));
  const schemas = $derived(app.getSchemas(connectionId, database.name));

  let expanded = $state(false);
  let expandedSchemas = $state<Set<string>>(new Set());

  const isSearching = $derived(filterQuery.length > 0);
  const dbPath = $derived(`${connectionId}/${database.name}`);
  const selfMatch = $derived(searchResults.get(dbPath));
  const hasChildMatch = $derived(app.hasDescendantMatch(dbPath, searchResults));
  const autoExpand = $derived(isSearching && hasChildMatch);

  // Tab-to-tree sync: expand once when active tab changes to target this database
  $effect(() => {
    const path = app.selectedObjectPath;
    if (path?.startsWith(dbPath + '/')) {
      expanded = true;
      // Extract and expand the target schema
      const schemaName = path.slice(dbPath.length + 1).split('/')[0];
      if (schemaName) {
        untrack(() => {
          if (!expandedSchemas.has(schemaName)) {
            expandedSchemas.add(schemaName);
            expandedSchemas = new Set(expandedSchemas);
          }
        });
      }
    }
  });

  // Auto-expand configured database + public schema on connect
  $effect(() => {
    if (isConfiguredDb && isDbConnected) {
      expanded = true;
      if (schemas.some(s => s.name === 'public') && !untrack(() => expandedSchemas.has('public'))) {
        expandedSchemas.add('public');
        expandedSchemas = new Set(expandedSchemas);
      }
    }
  });

  const sortedSchemas = $derived(
    [...schemas].sort((a, b) => {
      const aSystem = SYSTEM_SCHEMAS.has(a.name);
      const bSystem = SYSTEM_SCHEMAS.has(b.name);
      if (aSystem !== bSystem) return aSystem ? 1 : -1;
      return a.name.localeCompare(b.name);
    })
  );

  const filteredSchemas = $derived(
    isSearching
      ? sortedSchemas.filter(s => {
          const schemaPath = `${dbPath}/${s.name}`;
          return searchResults.has(schemaPath) || app.hasDescendantMatch(schemaPath, searchResults);
        })
      : sortedSchemas
  );

  function isSchemaExpanded(schemaName: string): boolean {
    if (isSearching) {
      const schemaPath = `${dbPath}/${schemaName}`;
      return app.hasDescendantMatch(schemaPath, searchResults);
    }
    return expandedSchemas.has(schemaName);
  }

  async function toggleDatabase() {
    if (!isDbConnected && !isDbConnecting) {
      expanded = true;
      await app.connectToSpecificDatabase(connectionId, database.name);
    } else {
      expanded = !expanded;
    }
  }

  function toggleSchema(schemaName: string) {
    if (expandedSchemas.has(schemaName)) {
      expandedSchemas.delete(schemaName);
    } else {
      expandedSchemas.add(schemaName);
    }
    expandedSchemas = new Set(expandedSchemas);
  }

  let showDropConfirm = $state(false);
  let showCreateDialog = $state(false);
  let showRenameDialog = $state(false);
  let showDbRestore = $state(false);
  let showSchemaRestore = $state(false);
  let restoreSchemaName = $state('');

  async function handleDropDatabase() {
    await app.dropDatabase(connectionId, database.name);
  }

  async function handleCreateDatabase(name: string) {
    await app.createDatabase(connectionId, name);
  }

  async function handleRenameDatabase(newName: string) {
    await app.renameDatabase(connectionId, database.name, newName);
  }

  async function handleRefresh() {
    await app.refreshDatabases(connectionId);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div oncontextmenu={(e) => e.stopPropagation()}>
  <ContextMenu.Root>
    <ContextMenu.Trigger class="block w-full">
      <!-- Database node -->
      <button
        class="w-full text-left pl-6 pr-2 py-0.5 text-xs font-medium flex items-center gap-1.5 hover:bg-sidebar-accent transition-colors"
        class:text-muted-foreground={database.is_template}
        onclick={toggleDatabase}
      >
        {#if isDbConnecting}
          <Loader2 class="h-3 w-3 text-muted-foreground animate-spin shrink-0" />
        {:else if (expanded || autoExpand) && isDbConnected}
          <ChevronDown class="h-3 w-3 text-muted-foreground shrink-0" />
        {:else}
          <ChevronRight class="h-3 w-3 text-muted-foreground shrink-0" />
        {/if}
        <Database class="h-3 w-3 shrink-0 {isDbConnected ? 'text-success' : 'text-muted-foreground'}" />
        {#if selfMatch}
          <HighlightMatch name={database.name} positions={selfMatch.positions} />
        {:else}
          <span class="truncate">{database.name}</span>
        {/if}
        {#if isConfiguredDb}
          <span class="text-muted-foreground text-[10px] ml-auto shrink-0">default</span>
        {/if}
      </button>

      <!-- Schema tree (when database is connected + expanded) -->
      {#if isDbConnected && (expanded || autoExpand)}
        {#each filteredSchemas as schema (schema.name)}
          {@const isSystem = SYSTEM_SCHEMAS.has(schema.name)}
          {@const schemaPath = `${dbPath}/${schema.name}`}
          {@const schemaMatch = searchResults.get(schemaPath)}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div oncontextmenu={(e) => e.stopPropagation()}>
            <ContextMenu.Root>
              <ContextMenu.Trigger class="block w-full">
                <button
                  class="w-full text-left pl-10 pr-2 py-0.5 text-xs flex items-center gap-1.5 hover:bg-sidebar-accent transition-colors"
                  class:text-muted-foreground={isSystem}
                  onclick={() => toggleSchema(schema.name)}
                >
                  {#if isSchemaExpanded(schema.name)}
                    <ChevronDown class="h-3 w-3 text-muted-foreground shrink-0" />
                    <FolderOpen class="h-3 w-3 text-warning shrink-0" />
                  {:else}
                    <ChevronRight class="h-3 w-3 text-muted-foreground shrink-0" />
                    <FolderClosed class="h-3 w-3 text-warning shrink-0" />
                  {/if}
                  {#if schemaMatch}
                    <HighlightMatch name={schema.name} positions={schemaMatch.positions} />
                  {:else}
                    <span class="truncate">{schema.name}</span>
                  {/if}
                </button>
              </ContextMenu.Trigger>
              <ContextMenu.Content>
                <ContextMenu.Item onclick={() => app.openErdTab(connectionId, database.name, schema.name)}>View ERD</ContextMenu.Item>
                <ContextMenu.Item onclick={() => app.openQueryTab(connectionId, database.name)}>New Query</ContextMenu.Item>
                <ContextMenu.Separator />
                <ContextMenu.Item onclick={() => { restoreSchemaName = schema.name; showSchemaRestore = true; }}>Restore from SQL...</ContextMenu.Item>
              </ContextMenu.Content>
            </ContextMenu.Root>
          </div>

          {#if isSchemaExpanded(schema.name)}
            <SchemaNode schemaName={schema.name} {connectionId} databaseName={database.name} {filterQuery} {searchResults} />
          {/if}
        {/each}
      {/if}
    </ContextMenu.Trigger>

    <ContextMenu.Content>
      {#if isDbConnected}
        <ContextMenu.Item onclick={() => app.openQueryTab(connectionId, database.name)}>
          New Query
        </ContextMenu.Item>
        <ContextMenu.Separator />
        <ContextMenu.Item onclick={() => (showDbRestore = true)}>
          Restore from SQL...
        </ContextMenu.Item>
        <ContextMenu.Separator />
        <ContextMenu.Item onclick={handleRefresh}>
          Refresh
        </ContextMenu.Item>
        <ContextMenu.Item onclick={() => app.disconnectSpecificDatabase(connectionId, database.name)}>
          Disconnect
        </ContextMenu.Item>
      {:else}
        <ContextMenu.Item onclick={() => app.connectToSpecificDatabase(connectionId, database.name)}>
          Connect
        </ContextMenu.Item>
      {/if}
      <ContextMenu.Separator />
      <ContextMenu.Item onclick={() => (showCreateDialog = true)}>
        New Database
      </ContextMenu.Item>
      <ContextMenu.Item onclick={() => (showRenameDialog = true)}>
        Rename Database
      </ContextMenu.Item>
      <ContextMenu.Separator />
      <ContextMenu.Item
        class="text-destructive focus:text-destructive"
        onclick={() => (showDropConfirm = true)}
      >
        Drop Database
      </ContextMenu.Item>
    </ContextMenu.Content>
  </ContextMenu.Root>
</div>

<ConfirmDialog
  bind:open={showDropConfirm}
  title="Drop Database"
  description={`This will permanently drop "${database.name}". This action cannot be undone.`}
  confirmLabel="Drop"
  variant="destructive"
  onconfirm={handleDropDatabase}
/>

<InputDialog
  bind:open={showCreateDialog}
  title="New Database"
  description="Enter a name for the new database."
  label="Database name"
  placeholder="my_database"
  confirmLabel="Create"
  onconfirm={handleCreateDatabase}
/>

<InputDialog
  bind:open={showRenameDialog}
  title="Rename Database"
  description={`Rename "${database.name}" to a new name.`}
  label="New name"
  placeholder={database.name}
  initialValue={database.name}
  confirmLabel="Rename"
  onconfirm={handleRenameDatabase}
/>

<RestoreDialog
  bind:open={showDbRestore}
  savedConnectionId={connectionId}
  databaseName={database.name}
/>

<RestoreDialog
  bind:open={showSchemaRestore}
  savedConnectionId={connectionId}
  databaseName={database.name}
  schema={restoreSchemaName}
/>
