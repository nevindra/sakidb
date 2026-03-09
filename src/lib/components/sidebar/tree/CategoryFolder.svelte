<script lang="ts">
  import { ChevronRight, ChevronDown, Loader2 } from '@lucide/svelte';
  import { untrack } from 'svelte';
  import type { Snippet } from 'svelte';
  import * as ContextMenu from '$lib/components/ui/context-menu';
  import { ContextMenuRenderer } from '$lib/context-menus';
  import type { MenuEntry, MenuContext } from '$lib/context-menus';

  let {
    label,
    count,
    icon: Icon,
    iconClass = 'text-muted-foreground',
    depth = 10,
    load,
    children,
    autoExpand = false,
    reveal = false,
    menuItems,
    menuCtx = {},
    onmenuaction,
  }: {
    label: string;
    count: number;
    icon: typeof ChevronRight;
    iconClass?: string;
    depth?: number;
    load: () => Promise<void>;
    children: Snippet;
    autoExpand?: boolean;
    reveal?: boolean;
    menuItems?: MenuEntry[];
    menuCtx?: MenuContext;
    onmenuaction?: (id: string) => void;
  } = $props();

  let expanded = $state(false);
  let loading = $state(false);
  let loaded = $state(false);

  const isExpanded = $derived(expanded || autoExpand);

  // Reveal: expand + load once when reveal becomes true (e.g. tab switch)
  // Only tracks `reveal` — user can still collapse manually
  $effect(() => {
    if (reveal) {
      untrack(() => {
        expanded = true;
        if (!loaded && !loading) {
          loading = true;
          load().then(() => { loaded = true; }).finally(() => { loading = false; });
        }
      });
    }
  });

  async function toggle() {
    if (expanded) {
      expanded = false;
      return;
    }
    expanded = true;
    if (!loaded) {
      loading = true;
      try {
        await load();
        loaded = true;
      } finally {
        loading = false;
      }
    }
  }
</script>

{#snippet folderButton()}
  <button
    class="w-full text-left pr-2 py-0.5 text-[11px] font-medium flex items-center gap-1 hover:bg-sidebar-accent transition-colors text-muted-foreground"
    style:padding-left="{depth * 4}px"
    onclick={toggle}
  >
    {#if loading}
      <Loader2 class="h-3 w-3 text-muted-foreground animate-spin shrink-0" />
    {:else if isExpanded}
      <ChevronDown class="h-3 w-3 text-muted-foreground shrink-0" />
    {:else}
      <ChevronRight class="h-3 w-3 text-muted-foreground shrink-0" />
    {/if}
    <Icon class="h-3 w-3 {iconClass} shrink-0" />
    <span class="truncate">{label}</span>
    {#if loaded}
      <span class="text-text-dim text-[10px] ml-auto shrink-0 tabular-nums">
        {count}
      </span>
    {/if}
  </button>
{/snippet}

{#if count > 0 || !loaded}
  {#if menuItems}
    <ContextMenu.Root>
      <ContextMenu.Trigger class="block w-full">
        {@render folderButton()}
      </ContextMenu.Trigger>
      <ContextMenuRenderer items={menuItems} ctx={menuCtx} onaction={(id) => onmenuaction?.(id)} />
    </ContextMenu.Root>
  {:else}
    {@render folderButton()}
  {/if}

  {#if isExpanded && loaded}
    {@render children()}
  {/if}
{/if}
