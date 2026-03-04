<script lang="ts">
  import { getAppState } from '$lib/stores';
  import { X, Plus, Table2, FileCode, Settings2, Workflow } from '@lucide/svelte';
  import { Button } from '$lib/components/ui/button';
  import * as ContextMenu from '$lib/components/ui/context-menu';
  import type { TabGroup, Tab } from '$lib/types';

  interface Props {
    group: TabGroup;
    isActive: boolean;
  }

  let { group, isActive }: Props = $props();

  const app = getAppState();

  const groupTabs = $derived(
    group.tabIds.map(id => app.getTabById(id)).filter((t): t is Tab => t != null)
  );

  function openNewQueryTab() {
    // Focus this group first so the new tab lands here
    app.setActiveGroup(group.id);
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

  // ── Drag state ──
  let dragTabId = $state<string | null>(null);
  let dragStartX = 0;
  let dragStartY = 0;
  let dragActive = $state(false);

  function handleDragStart(e: PointerEvent, tabId: string) {
    if (e.button !== 0) return;
    dragTabId = tabId;
    dragStartX = e.clientX;
    dragStartY = e.clientY;
    dragActive = false;

    const onMove = (ev: PointerEvent) => {
      const dx = ev.clientX - dragStartX;
      const dy = ev.clientY - dragStartY;
      if (!dragActive && Math.sqrt(dx * dx + dy * dy) > 5) {
        dragActive = true;
        // Dispatch custom event for SplitPane to pick up
        window.dispatchEvent(new CustomEvent('tab-drag-start', {
          detail: { tabId, sourceGroupId: group.id, clientX: ev.clientX, clientY: ev.clientY }
        }));
      }
      if (dragActive) {
        window.dispatchEvent(new CustomEvent('tab-drag-move', {
          detail: { tabId, clientX: ev.clientX, clientY: ev.clientY }
        }));
      }
    };

    const onUp = (ev: PointerEvent) => {
      if (dragActive) {
        window.dispatchEvent(new CustomEvent('tab-drag-end', {
          detail: { tabId, sourceGroupId: group.id, clientX: ev.clientX, clientY: ev.clientY }
        }));
      }
      dragTabId = null;
      dragActive = false;
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
    };

    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
  }
</script>

<div class="flex items-center bg-card border-b border-border h-8 shrink-0">
  <div class="flex-1 flex items-center overflow-x-auto">
    {#each groupTabs as tab (tab.id)}
      <ContextMenu.Root>
        <ContextMenu.Trigger class="shrink-0">
          <button
            class="flex items-center gap-1.5 px-3 h-8 text-xs border-r border-border shrink-0 transition-colors group relative"
            class:bg-background={tab.id === group.activeTabId}
            class:text-foreground={tab.id === group.activeTabId}
            class:bg-card={tab.id !== group.activeTabId}
            class:text-muted-foreground={tab.id !== group.activeTabId}
            onclick={() => app.setActiveTab(tab.id, group.id)}
            onpointerdown={(e) => handleDragStart(e, tab.id)}
            onmousedown={(e) => handleMiddleClick(e, tab.id)}
          >
            {#if tab.id === group.activeTabId}
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
              class:opacity-100={tab.id === group.activeTabId}
              role="button"
              tabindex={0}
              onclick={(e: MouseEvent) => { e.stopPropagation(); app.closeTab(tab.id); }}
              onkeydown={(e: KeyboardEvent) => { if (e.key === 'Enter') { e.stopPropagation(); app.closeTab(tab.id); } }}
            >
              <X class="h-3 w-3" />
            </span>
          </button>
        </ContextMenu.Trigger>
        <ContextMenu.Content>
          <ContextMenu.Item onclick={() => app.splitGroup(group.id, 'horizontal', tab.id)}>
            Split Right
          </ContextMenu.Item>
          <ContextMenu.Item onclick={() => app.splitGroup(group.id, 'vertical', tab.id)}>
            Split Down
          </ContextMenu.Item>
          <ContextMenu.Separator />
          <ContextMenu.Item onclick={() => app.closeTab(tab.id)}>
            Close Tab
          </ContextMenu.Item>
          <ContextMenu.Item onclick={() => {
            for (const t of groupTabs) {
              if (t.id !== tab.id) app.closeTab(t.id);
            }
          }}>
            Close Other Tabs
          </ContextMenu.Item>
          <ContextMenu.Item onclick={() => {
            for (const t of groupTabs) app.closeTab(t.id);
          }}>
            Close All Tabs
          </ContextMenu.Item>
        </ContextMenu.Content>
      </ContextMenu.Root>
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
