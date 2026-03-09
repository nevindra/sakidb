<script lang="ts">
  import type { MenuEntry, MenuContext } from './types';
  import * as ContextMenu from '$lib/components/ui/context-menu';

  let {
    items,
    ctx,
    onaction,
  }: {
    items: MenuEntry[];
    ctx: MenuContext;
    onaction: (id: string) => void;
  } = $props();

  const visible = $derived(items.filter(entry => !entry.when || entry.when(ctx)));
</script>

<ContextMenu.Content>
  {#each visible as entry}
    {#if 'kind' in entry}
      <ContextMenu.Separator />
    {:else}
      <ContextMenu.Item
        variant={entry.variant}
        onclick={() => onaction(entry.id)}
      >
        {entry.label}
      </ContextMenu.Item>
    {/if}
  {/each}
</ContextMenu.Content>
