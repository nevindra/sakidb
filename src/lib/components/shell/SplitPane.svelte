<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { getAppState } from '$lib/stores';
  import type { LayoutNode, SplitNode, TabGroup, Tab, DataTab, QueryTab, StructureTab, ErdTab } from '$lib/types';
  import { getCollapsingGroupId, finishCollapse, splitGroup } from '$lib/stores/layout.svelte';
  import { getGroupContainingTab, moveTab } from '$lib/stores/layout.svelte';
  import SplitPane from './SplitPane.svelte';
  import ResizeHandle from './ResizeHandle.svelte';
  import TabGroupBar from './TabGroupBar.svelte';
  import DropOverlay from './DropOverlay.svelte';
  import DataTabView from '$lib/components/data-view/DataTabView.svelte';
  import StructureTabView from '$lib/components/structure/StructureTabView.svelte';

  const LazyQueryTabView = import('$lib/components/query-editor/QueryTabView.svelte');
  const LazyErdTabView = import('$lib/components/erd/ErdTabView.svelte');

  interface Props {
    node: LayoutNode;
  }

  let { node }: Props = $props();

  const app = getAppState();

  // ── Split node handling ──
  const isSplit = $derived(node.type === 'split');
  const splitNode = $derived(isSplit ? node as SplitNode : null);

  // ── Tab group handling ──
  const isGroup = $derived(node.type === 'tab-group');
  const groupNode = $derived(isGroup ? node as TabGroup : null);
  const isActiveGroup = $derived(groupNode ? groupNode.id === app.activeGroupId : false);

  const activeTab = $derived.by(() => {
    if (!groupNode?.activeTabId) return undefined;
    return app.getTabById(groupNode.activeTabId);
  });

  // ── Collapse animation ──
  const collapsingId = $derived(getCollapsingGroupId());

  let paneRef: HTMLDivElement | undefined = $state();

  // ── Drag-and-drop zone detection ──
  let dropZone = $state<'top' | 'right' | 'bottom' | 'left' | 'center' | null>(null);

  function getDropZone(clientX: number, clientY: number): 'top' | 'right' | 'bottom' | 'left' | 'center' | null {
    if (!paneRef || !groupNode) return null;
    const rect = paneRef.getBoundingClientRect();
    const x = clientX - rect.left;
    const y = clientY - rect.top;
    const edgeSize = 40;

    if (y < edgeSize) return 'top';
    if (y > rect.height - edgeSize) return 'bottom';
    if (x < edgeSize) return 'left';
    if (x > rect.width - edgeSize) return 'right';
    return 'center';
  }

  // Listen for tab drag events
  let dragSourceGroupId: string | null = null;
  let dragTabId: string | null = null;

  function onTabDragStart(e: Event) {
    const detail = (e as CustomEvent).detail;
    dragSourceGroupId = detail.sourceGroupId;
    dragTabId = detail.tabId;
    document.body.style.userSelect = 'none';
  }

  function onTabDragMove(e: Event) {
    const detail = (e as CustomEvent).detail;
    if (!groupNode) return;
    dropZone = getDropZone(detail.clientX, detail.clientY);
  }

  function onTabDragEnd(e: Event) {
    const detail = (e as CustomEvent).detail;
    document.body.style.userSelect = '';

    if (!groupNode || !dropZone || !detail.tabId) {
      dropZone = null;
      return;
    }

    const tabId = detail.tabId;
    const sourceGroupId = detail.sourceGroupId;

    if (dropZone === 'center') {
      // Move tab into this group
      if (sourceGroupId !== groupNode.id) {
        moveTab(tabId, sourceGroupId, groupNode.id);
      }
    } else if (dropZone === 'right' || dropZone === 'left') {
      // Create horizontal split
      if (sourceGroupId === groupNode.id) {
        splitGroup(groupNode.id, 'horizontal', tabId);
      } else {
        // Move to this group first, then split
        moveTab(tabId, sourceGroupId, groupNode.id);
        splitGroup(groupNode.id, 'horizontal', tabId);
      }
    } else if (dropZone === 'top' || dropZone === 'bottom') {
      if (sourceGroupId === groupNode.id) {
        splitGroup(groupNode.id, 'vertical', tabId);
      } else {
        moveTab(tabId, sourceGroupId, groupNode.id);
        splitGroup(groupNode.id, 'vertical', tabId);
      }
    }

    dropZone = null;
    dragTabId = null;
    dragSourceGroupId = null;
  }

  onMount(() => {
    window.addEventListener('tab-drag-start', onTabDragStart);
    window.addEventListener('tab-drag-move', onTabDragMove);
    window.addEventListener('tab-drag-end', onTabDragEnd);
  });

  onDestroy(() => {
    window.removeEventListener('tab-drag-start', onTabDragStart);
    window.removeEventListener('tab-drag-move', onTabDragMove);
    window.removeEventListener('tab-drag-end', onTabDragEnd);
  });

  // ── Collapse animation handling ──
  let collapsing = $state(false);

  function handleTransitionEnd() {
    if (collapsing && groupNode) {
      collapsing = false;
      finishCollapse(groupNode.id);
    }
  }

  $effect(() => {
    if (groupNode && collapsingId === groupNode.id) {
      collapsing = true;
    }
  });

  // ── Resize state for will-change optimization ──
  let resizing = $state(false);
</script>

{#if splitNode}
  <!-- Split node: render two children with a resize handle -->
  <div
    class="flex h-full w-full overflow-hidden"
    style:flex-direction={splitNode.direction === 'horizontal' ? 'row' : 'column'}
  >
    <div
      class="overflow-hidden min-w-0 min-h-0"
      style:flex-basis="{splitNode.ratio * 100}%"
      style:flex-grow="0"
      style:flex-shrink="0"
      style:will-change={resizing ? 'flex-basis' : 'auto'}
    >
      <SplitPane node={splitNode.first} />
    </div>

    <ResizeHandle
      direction={splitNode.direction}
      onResize={(ratio) => app.resizeSplit(splitNode!.id, ratio)}
      onResizeStart={() => resizing = true}
      onResizeEnd={() => resizing = false}
    />

    <div
      class="overflow-hidden min-w-0 min-h-0"
      style:flex-basis="{(1 - splitNode.ratio) * 100}%"
      style:flex-grow="0"
      style:flex-shrink="0"
      style:will-change={resizing ? 'flex-basis' : 'auto'}
    >
      <SplitPane node={splitNode.second} />
    </div>
  </div>
{:else if groupNode}
  <!-- Leaf node: tab group with tab bar + content -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    bind:this={paneRef}
    class="flex flex-col h-full w-full overflow-hidden relative"
    class:ring-1={isActiveGroup}
    class:ring-primary={isActiveGroup}
    style:--tw-ring-opacity={isActiveGroup ? '0.3' : undefined}
    class:transition-[flex-basis]={collapsing}
    class:duration-150={collapsing}
    class:ease-out={collapsing}
    onclick={() => app.setActiveGroup(groupNode!.id)}
    ontransitionend={handleTransitionEnd}
  >
    <TabGroupBar group={groupNode} isActive={isActiveGroup} />

    {#if activeTab}
      {#key activeTab.id}
        <div class="flex-1 overflow-hidden min-h-0">
          {#if activeTab.type === 'data'}
            <DataTabView tab={activeTab as DataTab} />
          {:else if activeTab.type === 'query'}
            {#await LazyQueryTabView then mod}
              <mod.default tab={activeTab as QueryTab} />
            {/await}
          {:else if activeTab.type === 'structure'}
            <StructureTabView tab={activeTab as StructureTab} />
          {:else if activeTab.type === 'erd'}
            {#await LazyErdTabView then mod}
              <mod.default tab={activeTab as ErdTab} />
            {/await}
          {/if}
        </div>
      {/key}
    {/if}

    <DropOverlay activeZone={dropZone} />
  </div>
{/if}
