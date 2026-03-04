<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { SavedConnection } from '$lib/types';
  import type { FuzzyResult } from '$lib/utils/fuzzy';
  import { ChevronRight, ChevronDown, Loader2, Server } from '@lucide/svelte';
  import * as ContextMenu from '$lib/components/ui/context-menu';
  import DatabaseNode from './tree/DatabaseNode.svelte';
  import HighlightMatch from './HighlightMatch.svelte';

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

  async function toggleConnection() {
    if (!isConnected && !isConnecting) {
      expanded = true;
      await app.connectToDatabase(connection.id);
    } else {
      expanded = !expanded;
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
    </button>

    <!-- Database tree (when connected + expanded) -->
    {#if isConnected && (expanded || autoExpand)}
      {#each filteredDatabases as db (db.name)}
        <DatabaseNode
          database={db}
          connectionId={connection.id}
          isConfiguredDb={db.name === connection.database}
          {filterQuery}
          {searchResults}
        />
      {/each}
    {/if}
  </ContextMenu.Trigger>

  <ContextMenu.Content>
    {#if isConnected}
      <ContextMenu.Item onclick={() => {
        app.openQueryTab(connection.id, connection.database);
      }}>
        New Query
      </ContextMenu.Item>
      <ContextMenu.Separator />
      <ContextMenu.Item onclick={() => app.disconnectFromDatabase(connection.id)}>
        Disconnect
      </ContextMenu.Item>
    {:else}
      <ContextMenu.Item onclick={() => app.connectToDatabase(connection.id)}>
        Connect
      </ContextMenu.Item>
    {/if}
    <ContextMenu.Separator />
    <ContextMenu.Item onclick={() => app.openEditDialog(connection.id)}>
      Edit
    </ContextMenu.Item>
    <ContextMenu.Item
      class="text-destructive focus:text-destructive"
      onclick={() => app.deleteConnection(connection.id)}
    >
      Delete
    </ContextMenu.Item>
  </ContextMenu.Content>
</ContextMenu.Root>
