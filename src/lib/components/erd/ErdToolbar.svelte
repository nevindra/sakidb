<script lang="ts">
  import { ZoomIn, ZoomOut, Maximize, RotateCcw, Download, Search, Layers, Layers2 } from '@lucide/svelte';
  import { Button } from '$lib/components/ui/button';
  import * as Tooltip from '$lib/components/ui/tooltip';

  let {
    zoom = 1,
    searchQuery = '',
    simplified = false,
    onzoomin,
    onzoomout,
    onfit,
    onrelayout,
    onexportpng,
    onexportsvg,
    onsearch,
    ontogglemode,
  }: {
    zoom?: number;
    searchQuery?: string;
    simplified?: boolean;
    onzoomin?: () => void;
    onzoomout?: () => void;
    onfit?: () => void;
    onrelayout?: () => void;
    onexportpng?: () => void;
    onexportsvg?: () => void;
    onsearch?: (query: string) => void;
    ontogglemode?: () => void;
  } = $props();

  let exportOpen = $state(false);

  function handleSearchInput(e: Event) {
    const value = (e.target as HTMLInputElement).value;
    onsearch?.(value);
  }
</script>

<div class="flex items-center gap-1 px-2 h-8 bg-card border-b border-border shrink-0">
  <!-- Search -->
  <div class="relative flex items-center">
    <Search class="absolute left-2 h-3 w-3 text-muted-foreground pointer-events-none" />
    <input
      type="text"
      placeholder="Search tables..."
      value={searchQuery}
      oninput={handleSearchInput}
      class="h-6 w-40 pl-7 pr-2 text-xs bg-secondary border border-border rounded-md text-foreground placeholder:text-text-dim focus:outline-none focus:border-primary"
    />
  </div>

  <div class="w-px h-4 bg-border mx-1"></div>

  <!-- Zoom controls -->
  <Button variant="ghost" size="icon-sm" class="h-6 w-6" onclick={onzoomout}>
    <ZoomOut class="h-3.5 w-3.5" />
  </Button>
  <span class="text-xs text-muted-foreground w-10 text-center tabular-nums">
    {Math.round(zoom * 100)}%
  </span>
  <Button variant="ghost" size="icon-sm" class="h-6 w-6" onclick={onzoomin}>
    <ZoomIn class="h-3.5 w-3.5" />
  </Button>
  <Button variant="ghost" size="icon-sm" class="h-6 w-6" onclick={onfit}>
    <Maximize class="h-3.5 w-3.5" />
  </Button>

  <div class="w-px h-4 bg-border mx-1"></div>

  <!-- Layout -->
  <Button variant="ghost" size="icon-sm" class="h-6 w-6" onclick={onrelayout}>
    <RotateCcw class="h-3.5 w-3.5" />
  </Button>

  <div class="w-px h-4 bg-border mx-1"></div>

  <!-- Simplified / Detailed toggle -->
  <Tooltip.Root>
    <Tooltip.Trigger>
      <Button
        variant={simplified ? 'secondary' : 'ghost'}
        size="sm"
        class="h-6 px-2 text-xs gap-1"
        onclick={ontogglemode}
      >
        {#if simplified}
          <Layers class="h-3 w-3" />
          Simplified
        {:else}
          <Layers2 class="h-3 w-3" />
          Detailed
        {/if}
      </Button>
    </Tooltip.Trigger>
    <Tooltip.Content>
      {simplified ? 'Showing parent tables only — partitions hidden' : 'Showing all tables including partitions'}
    </Tooltip.Content>
  </Tooltip.Root>

  <div class="flex-1"></div>

  <!-- Export -->
  <div class="relative">
    <Button variant="ghost" size="icon-sm" class="h-6 w-6" onclick={() => (exportOpen = !exportOpen)}>
      <Download class="h-3.5 w-3.5" />
    </Button>
    {#if exportOpen}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="absolute right-0 top-7 z-50 bg-popover border border-border rounded-md shadow-lg py-1 min-w-[100px]"
        onclick={() => (exportOpen = false)}
      >
        <button class="w-full text-left px-3 py-1 text-xs hover:bg-accent transition-colors" onclick={onexportpng}>
          Export PNG
        </button>
        <button class="w-full text-left px-3 py-1 text-xs hover:bg-accent transition-colors" onclick={onexportsvg}>
          Export SVG
        </button>
      </div>
    {/if}
  </div>
</div>
