<script lang="ts">
  import type { SequenceInfo, IndexInfo, ForeignTableInfo } from '$lib/types';
  import type { FuzzyResult } from '$lib/utils/fuzzy';
  import { Hash, ListTree, ExternalLink, ChevronRight, ChevronDown } from '@lucide/svelte';
  import * as ContextMenu from '$lib/components/ui/context-menu';
  import HighlightMatch from '../HighlightMatch.svelte';

  type ObjectItem =
    | { kind: 'sequence'; data: SequenceInfo }
    | { kind: 'index'; data: IndexInfo }
    | { kind: 'foreign_table'; data: ForeignTableInfo };

  let {
    item,
    schema,
    depth = 14,
    searchResults = new Map(),
    schemaPrefix = '',
  }: {
    item: ObjectItem;
    schema: string;
    depth?: number;
    searchResults?: Map<string, FuzzyResult>;
    schemaPrefix?: string;
  } = $props();

  let expanded = $state(false);

  const Icon = $derived(
    item.kind === 'sequence' ? Hash
    : item.kind === 'index' ? ListTree
    : ExternalLink
  );

  const iconClass = $derived(
    item.kind === 'sequence' ? 'text-orange-400'
    : item.kind === 'index' ? 'text-teal-400'
    : 'text-rose-400'
  );

  const name = $derived(item.data.name);

  const selfMatch = $derived(schemaPrefix ? searchResults.get(`${schemaPrefix}/${name}`) : undefined);

  const subtitle = $derived(
    item.kind === 'sequence' ? (item.data as SequenceInfo).data_type
    : item.kind === 'index' ? (item.data as IndexInfo).index_type
    : (item.data as ForeignTableInfo).server_name
  );
</script>

<ContextMenu.Root>
  <ContextMenu.Trigger class="block w-full">
    <button
      class="w-full text-left pr-2 py-0.5 text-xs flex items-center gap-1.5 hover:bg-sidebar-accent transition-colors"
      style:padding-left="{depth * 4}px"
      onclick={() => expanded = !expanded}
    >
      {#if expanded}
        <ChevronDown class="h-3 w-3 text-muted-foreground shrink-0" />
      {:else}
        <ChevronRight class="h-3 w-3 text-muted-foreground shrink-0" />
      {/if}
      <Icon class="h-3 w-3 {iconClass} shrink-0" />
      {#if selfMatch}
        <HighlightMatch name={name} positions={selfMatch.positions} />
      {:else}
        <span class="truncate">{name}</span>
      {/if}
      <span class="text-text-dim text-[10px] ml-auto shrink-0">{subtitle}</span>
    </button>
  </ContextMenu.Trigger>
  <ContextMenu.Content>
    <ContextMenu.Item onclick={() => navigator.clipboard.writeText(`"${schema}"."${name}"`)}>
      Copy Name
    </ContextMenu.Item>
  </ContextMenu.Content>
</ContextMenu.Root>

{#if expanded}
  <div
    class="pr-2 py-0.5 text-xs text-muted-foreground space-y-0.5"
    style:padding-left="{(depth + 3) * 4}px"
  >
    {#if item.kind === 'sequence'}
      {@const seq = item.data as SequenceInfo}
      <div><span class="text-text-dim">type:</span> {seq.data_type}</div>
      {#if seq.last_value != null}
        <div><span class="text-text-dim">last value:</span> {seq.last_value}</div>
      {/if}
    {:else if item.kind === 'index'}
      {@const idx = item.data as IndexInfo}
      <div><span class="text-text-dim">table:</span> {idx.table_name}</div>
      <div><span class="text-text-dim">columns:</span> {idx.columns}</div>
      <div><span class="text-text-dim">type:</span> {idx.index_type}{idx.is_unique ? ', unique' : ''}{idx.is_primary ? ', primary' : ''}</div>
    {:else}
      {@const ft = item.data as ForeignTableInfo}
      <div><span class="text-text-dim">server:</span> {ft.server_name}</div>
    {/if}
  </div>
{/if}
