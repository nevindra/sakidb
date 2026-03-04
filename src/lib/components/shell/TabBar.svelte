<script lang="ts">
  import { getAppState } from '$lib/stores';
  import { X, Plus, Table2, FileCode, Settings2, Workflow } from '@lucide/svelte';
  import { Button } from '$lib/components/ui/button';

  const app = getAppState();

  // Find a connected connection to open a query tab for
  function openNewQueryTab() {
    const firstConnected = [...app.activeConnections.values()][0];
    if (firstConnected) {
      const firstDb = [...firstConnected.activeDatabases.keys()][0];
      if (firstDb) {
        app.openQueryTab(firstConnected.savedConnectionId, firstDb);
      }
    }
  }

  function handleMiddleClick(e: MouseEvent, tabId: string) {
    if (e.button === 1) {
      e.preventDefault();
      app.closeTab(tabId);
    }
  }
</script>

{#if app.tabs.length > 0}
  <div class="flex items-center bg-card border-b border-border h-8 shrink-0">
    <div class="flex-1 flex items-center overflow-x-auto">
      {#each app.tabs as tab (tab.id)}
        <button
          class="flex items-center gap-1.5 px-3 h-8 text-xs border-r border-border shrink-0 transition-colors group relative"
          class:bg-background={tab.id === app.activeTabId}
          class:text-foreground={tab.id === app.activeTabId}
          class:bg-card={tab.id !== app.activeTabId}
          class:text-muted-foreground={tab.id !== app.activeTabId}
          onclick={() => app.setActiveTab(tab.id)}
          onmousedown={(e) => handleMiddleClick(e, tab.id)}
        >
          {#if tab.id === app.activeTabId}
            <span class="absolute bottom-0 left-0 right-0 h-0.5 bg-primary"></span>
          {/if}

          {#if tab.type === 'data'}
            <Table2 class="h-3 w-3 text-primary shrink-0" />
            <span class="truncate max-w-[160px]">{tab.schema}.{tab.table}</span>
            <span class="text-text-dim text-[10px]">{tab.connectionName}</span>
          {:else if tab.type === 'structure'}
            <Settings2 class="h-3 w-3 text-emerald-400 shrink-0" />
            <span class="truncate max-w-[160px]">{tab.schema}.{tab.table}</span>
            <span class="text-text-dim text-[10px]">structure</span>
          {:else if tab.type === 'erd'}
            <Workflow class="h-3 w-3 text-violet-400 shrink-0" />
            <span class="truncate max-w-[160px]">ERD: {tab.schema}</span>
            <span class="text-text-dim text-[10px]">{tab.connectionName}</span>
          {:else}
            <FileCode class="h-3 w-3 text-info shrink-0" />
            <span class="truncate max-w-[120px]">{tab.title}</span>
            <span class="text-text-dim text-[10px]">{tab.connectionName}</span>
          {/if}

          <span
            class="ml-1 opacity-0 group-hover:opacity-100 hover:text-destructive transition-opacity shrink-0 cursor-pointer"
            class:opacity-100={tab.id === app.activeTabId}
            role="button"
            tabindex={0}
            onclick={(e: MouseEvent) => { e.stopPropagation(); app.closeTab(tab.id); }}
            onkeydown={(e: KeyboardEvent) => { if (e.key === 'Enter') { e.stopPropagation(); app.closeTab(tab.id); } }}
          >
            <X class="h-3 w-3" />
          </span>
        </button>
      {/each}

      {#if app.activeConnections.size > 0}
        <Button
          variant="ghost"
          size="icon-sm"
          class="h-8 w-8 shrink-0 text-muted-foreground hover:text-foreground rounded-none border-l border-border"
          onclick={openNewQueryTab}
        >
          <Plus class="h-3.5 w-3.5" />
        </Button>
      {/if}
    </div>
  </div>
{/if}
