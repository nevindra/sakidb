<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { ViewInfo } from '$lib/types';
  import type { FuzzyResult } from '$lib/utils/fuzzy';
  import { Eye } from '@lucide/svelte';
  import * as ContextMenu from '$lib/components/ui/context-menu';
  import { ContextMenuRenderer, viewMenuItems } from '$lib/context-menus';
  import HighlightMatch from '../HighlightMatch.svelte';
  import { getDialect } from '$lib/dialects';

  let {
    view,
    schema,
    connectionId,
    databaseName,
    depth = 14,
    searchResults = new Map(),
    schemaPrefix = '',
  }: {
    view: ViewInfo;
    schema: string;
    connectionId: string;
    databaseName: string;
    depth?: number;
    searchResults?: Map<string, FuzzyResult>;
    schemaPrefix?: string;
  } = $props();

  const selfMatch = $derived(schemaPrefix ? searchResults.get(`${schemaPrefix}/${view.name}`) : undefined);

  const app = getAppState();
  const dialect = $derived((() => { const e = app.getSavedConnection(connectionId)?.engine; return e ? getDialect(e as import('$lib/types').EngineType) : null; })());

  function handleClick() {
    app.openDataTab(connectionId, databaseName, schema, view.name);
  }

  function handleMenuAction(id: string) {
    switch (id) {
      case 'open-data': return handleClick();
      case 'new-query': return app.openQueryTab(connectionId, databaseName,
        `SELECT * FROM ${dialect?.qualifiedTable(schema, view.name) ?? '"' + view.name + '"'} LIMIT 100;`);
      case 'copy-name': return navigator.clipboard.writeText(
        dialect?.qualifiedTable(schema, view.name) ?? `"${schema}"."${view.name}"`);
    }
  }
</script>

<ContextMenu.Root>
  <ContextMenu.Trigger class="block w-full">
    <button
      class="w-full text-left pr-2 py-0.5 text-xs flex items-center gap-1.5 hover:bg-sidebar-accent transition-colors"
      style:padding-left="{depth * 4}px"
      onclick={handleClick}
    >
      <Eye class="h-3 w-3 text-sky-400 shrink-0" />
      {#if selfMatch}
        <HighlightMatch name={view.name} positions={selfMatch.positions} />
      {:else}
        <span class="truncate">{view.name}</span>
      {/if}
    </button>
  </ContextMenu.Trigger>
  <ContextMenuRenderer items={viewMenuItems()} ctx={{}} onaction={handleMenuAction} />
</ContextMenu.Root>
