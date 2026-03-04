<script lang="ts">
  import type { FuzzyResult } from '$lib/utils/fuzzy';
  import { Star, Copy } from '@lucide/svelte';
  import * as ContextMenu from '$lib/components/ui/context-menu';
  import * as Tooltip from '$lib/components/ui/tooltip';
  import HighlightMatch from './HighlightMatch.svelte';

  let {
    sql,
    name = '',
    timestamp = '',
    isSaved = false,
    nameMatch,
    onOpen,
    onSave,
    onEdit,
    onDelete,
    onCopySql,
  }: {
    sql: string;
    name?: string;
    timestamp?: string;
    isSaved?: boolean;
    nameMatch?: FuzzyResult;
    onOpen: () => void;
    onSave?: () => void;
    onEdit?: () => void;
    onDelete?: () => void;
    onCopySql: () => void;
  } = $props();

  const truncatedSql = $derived(
    sql.length > 80 ? sql.slice(0, 80).replace(/\s+/g, ' ') + '\u2026' : sql.replace(/\s+/g, ' ')
  );

  function relativeTime(iso: string): string {
    const diff = Date.now() - new Date(iso).getTime();
    const seconds = Math.floor(diff / 1000);
    if (seconds < 60) return 'just now';
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m ago`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ago`;
    const days = Math.floor(hours / 24);
    return `${days}d ago`;
  }
</script>

<ContextMenu.Root>
  <ContextMenu.Trigger class="block w-full">
    <button
      class="group flex items-start gap-2 w-full px-3 py-1.5 text-left hover:bg-sidebar-accent/50 transition-colors"
      onclick={onOpen}
    >
      <div class="flex-1 min-w-0">
        {#if isSaved && name}
          <div class="text-xs text-sidebar-foreground truncate">
            {#if nameMatch}
              <HighlightMatch {name} positions={nameMatch.positions} />
            {:else}
              {name}
            {/if}
          </div>
          <div class="text-[11px] text-muted-foreground truncate font-mono">{truncatedSql}</div>
        {:else}
          <div class="text-xs text-sidebar-foreground truncate font-mono">{truncatedSql}</div>
          {#if timestamp}
            <div class="text-[11px] text-muted-foreground">{relativeTime(timestamp)}</div>
          {/if}
        {/if}
      </div>

      {#if isSaved}
        <Tooltip.Root>
          <Tooltip.Trigger>
            {#snippet child({ props })}
              <button
                {...props}
                class="flex-shrink-0 mt-0.5 p-0.5 rounded opacity-0 group-hover:opacity-100 transition-opacity text-muted-foreground hover:text-foreground"
                onclick={(e) => { e.stopPropagation(); onEdit?.(); }}
              >
                <svg class="h-3 w-3" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z"/></svg>
              </button>
            {/snippet}
          </Tooltip.Trigger>
          <Tooltip.Content>Edit</Tooltip.Content>
        </Tooltip.Root>
      {:else}
        <Tooltip.Root>
          <Tooltip.Trigger>
            {#snippet child({ props })}
              <button
                {...props}
                class="flex-shrink-0 mt-0.5 p-0.5 rounded opacity-0 group-hover:opacity-100 transition-opacity text-muted-foreground hover:text-yellow-400"
                onclick={(e) => { e.stopPropagation(); onSave?.(); }}
              >
                <Star class="h-3 w-3" />
              </button>
            {/snippet}
          </Tooltip.Trigger>
          <Tooltip.Content>Save query</Tooltip.Content>
        </Tooltip.Root>
      {/if}
    </button>
  </ContextMenu.Trigger>

  <ContextMenu.Content>
    <ContextMenu.Item onclick={onOpen}>
      Open in New Tab
    </ContextMenu.Item>
    <ContextMenu.Item onclick={onCopySql}>
      <Copy class="h-3.5 w-3.5 mr-2" />
      Copy SQL
    </ContextMenu.Item>
    {#if isSaved}
      <ContextMenu.Separator />
      <ContextMenu.Item onclick={onEdit}>
        Edit
      </ContextMenu.Item>
      <ContextMenu.Item
        class="text-destructive focus:text-destructive"
        onclick={onDelete}
      >
        Delete
      </ContextMenu.Item>
    {:else}
      <ContextMenu.Separator />
      <ContextMenu.Item onclick={onSave}>
        <Star class="h-3.5 w-3.5 mr-2" />
        Save Query
      </ContextMenu.Item>
    {/if}
  </ContextMenu.Content>
</ContextMenu.Root>
