<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { FuzzyResult } from '$lib/utils/fuzzy';
  import ConnectionTree from './ConnectionTree.svelte';
  import QueryList from './QueryList.svelte';
  import { Search, X } from '@lucide/svelte';
  import * as Tooltip from '$lib/components/ui/tooltip';
  import { Button } from '$lib/components/ui/button';

  const app = getAppState();

  let activeTab = $state<'connections' | 'queries'>('connections');
  let searchQuery = $state('');
  let showSearch = $state(false);

  function switchTab(tab: 'connections' | 'queries') {
    activeTab = tab;
    searchQuery = '';
    showSearch = false;
  }

  const searchResults = $derived<Map<string, FuzzyResult>>(
    searchQuery ? app.searchTree(searchQuery) : new Map()
  );

  const filteredConnections = $derived(
    searchQuery
      ? app.savedConnections.filter(c =>
          searchResults.has(c.id) || app.hasDescendantMatch(c.id, searchResults)
        )
      : app.savedConnections
  );
</script>

<div class="flex flex-col h-full bg-sidebar border-r border-sidebar-border">
  <!-- Tab switcher -->
  <div class="flex items-center border-b border-sidebar-border">
    <div class="flex flex-1">
      <button
        class="flex-1 px-3 py-2 text-[11px] font-semibold uppercase tracking-wide transition-colors border-b-2 {activeTab === 'connections' ? 'text-foreground border-primary' : 'text-muted-foreground border-transparent hover:text-foreground/70'}"
        onclick={() => switchTab('connections')}
      >
        Connections
      </button>
      <button
        class="flex-1 px-3 py-2 text-[11px] font-semibold uppercase tracking-wide transition-colors border-b-2 {activeTab === 'queries' ? 'text-foreground border-primary' : 'text-muted-foreground border-transparent hover:text-foreground/70'}"
        onclick={() => switchTab('queries')}
      >
        Queries
      </button>
    </div>
    <div class="flex items-center gap-0.5 pr-2">
      <Tooltip.Root>
        <Tooltip.Trigger>
          {#snippet child({ props })}
            <Button
              {...props}
              variant="ghost"
              size="icon-sm"
              class="h-6 w-6 text-muted-foreground hover:text-foreground"
              onclick={() => { showSearch = !showSearch; if (!showSearch) searchQuery = ''; }}
            >
              <Search class="h-3.5 w-3.5" />
            </Button>
          {/snippet}
        </Tooltip.Trigger>
        <Tooltip.Content>Filter</Tooltip.Content>
      </Tooltip.Root>
    </div>
  </div>

  <!-- Search input -->
  {#if showSearch}
    <div class="px-2 py-1.5 border-b border-sidebar-border">
      <div class="relative">
        <!-- svelte-ignore a11y_autofocus -->
        <input
          type="text"
          bind:value={searchQuery}
          placeholder="Filter..."
          class="w-full px-2 py-1 pr-6 text-xs bg-sidebar-accent border border-sidebar-border rounded text-sidebar-foreground placeholder:text-muted-foreground focus:outline-none focus:border-ring"
          autofocus
        />
        {#if searchQuery}
          <button
            class="absolute right-1 top-1/2 -translate-y-1/2 p-0.5 rounded text-muted-foreground hover:text-foreground transition-colors"
            onclick={() => searchQuery = ''}
          >
            <X class="h-3 w-3" />
          </button>
        {/if}
      </div>
    </div>
  {/if}

  <!-- Tab content -->
  <div class="flex-1 overflow-y-auto py-1">
    {#if activeTab === 'connections'}
      {#each filteredConnections as conn (conn.id)}
        <ConnectionTree connection={conn} filterQuery={searchQuery} {searchResults} />
      {/each}
    {:else}
      <QueryList filterQuery={searchQuery} />
    {/if}
  </div>
</div>
