<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { MaterializedViewInfo } from '$lib/types';
  import type { FuzzyResult } from '$lib/utils/fuzzy';
  import { Layers } from '@lucide/svelte';
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
    view: MaterializedViewInfo;
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

  function formatRowCount(count: number | null): string {
    if (count === null) return '';
    if (count >= 1_000_000) return `~${(count / 1_000_000).toFixed(1)}M`;
    if (count >= 1_000) return `~${(count / 1_000).toFixed(1)}k`;
    return `~${count}`;
  }
</script>

<ContextMenu.Root>
  <ContextMenu.Trigger class="block w-full">
    <button
      class="w-full text-left pr-2 py-0.5 text-xs flex items-center gap-1.5 hover:bg-sidebar-accent transition-colors"
      style:padding-left="{depth * 4}px"
      onclick={handleClick}
    >
      <Layers class="h-3 w-3 text-violet-400 shrink-0" />
      {#if selfMatch}
        <HighlightMatch name={view.name} positions={selfMatch.positions} />
      {:else}
        <span class="truncate">{view.name}</span>
      {/if}
      {#if !view.is_populated}
        <span class="text-warning text-[10px] ml-auto shrink-0">unpopulated</span>
      {:else if view.row_count_estimate != null}
        <span class="text-muted-foreground text-[10px] ml-auto shrink-0 tabular-nums">
          {formatRowCount(view.row_count_estimate)}
        </span>
      {/if}
    </button>
  </ContextMenu.Trigger>
  <ContextMenu.Content>
    <ContextMenu.Item onclick={handleClick}>
      Open Data
    </ContextMenu.Item>
    <ContextMenu.Item onclick={() => {
      app.openQueryTab(connectionId, databaseName, `REFRESH MATERIALIZED VIEW "${schema}"."${view.name}";`);
    }}>
      Refresh
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
