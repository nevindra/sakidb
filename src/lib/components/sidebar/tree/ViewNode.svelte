<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { ViewInfo } from '$lib/types';
  import type { FuzzyResult } from '$lib/utils/fuzzy';
  import { Eye } from '@lucide/svelte';
  import * as ContextMenu from '$lib/components/ui/context-menu';
  import HighlightMatch from '../HighlightMatch.svelte';

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

  function handleClick() {
    app.openDataTab(connectionId, databaseName, schema, view.name);
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
  <ContextMenu.Content>
    <ContextMenu.Item onclick={handleClick}>
      Open Data
    </ContextMenu.Item>
    <ContextMenu.Item onclick={() => {
      app.openQueryTab(connectionId, databaseName, `SELECT * FROM "${schema}"."${view.name}" LIMIT 100;`);
    }}>
      New Query
    </ContextMenu.Item>
    <ContextMenu.Separator />
    <ContextMenu.Item onclick={() => navigator.clipboard.writeText(`"${schema}"."${view.name}"`)}>
      Copy Qualified Name
    </ContextMenu.Item>
  </ContextMenu.Content>
</ContextMenu.Root>
