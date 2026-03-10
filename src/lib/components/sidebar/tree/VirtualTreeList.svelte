<script lang="ts" generics="T">
  import type { Snippet } from 'svelte';

  let {
    items,
    getKey,
    rowHeight = 24,
    threshold = 100,
    maxHeight = '70vh',
    overscan = 20,
    children,
  }: {
    items: T[];
    getKey: (item: T) => string;
    rowHeight?: number;
    threshold?: number;
    maxHeight?: string;
    overscan?: number;
    children: Snippet<[T, number]>;
  } = $props();

  let scrollTop = $state(0);
  let containerHeight = $state(600); // sensible default until measured

  const shouldVirtualize = $derived(items.length > threshold);
  const totalHeight = $derived(items.length * rowHeight);

  const startIndex = $derived(
    Math.max(0, Math.floor(scrollTop / rowHeight) - overscan)
  );
  const endIndex = $derived(
    Math.min(items.length, Math.ceil((scrollTop + containerHeight) / rowHeight) + overscan)
  );
  const visibleItems = $derived(
    items.slice(startIndex, endIndex).map((item, i) => ({
      item,
      index: startIndex + i,
    }))
  );
  const offsetY = $derived(startIndex * rowHeight);

  function onScroll(e: Event) {
    const el = e.currentTarget as HTMLDivElement;
    scrollTop = el.scrollTop;
    containerHeight = el.clientHeight;
  }

  function measureHeight(node: HTMLDivElement) {
    containerHeight = node.clientHeight;
  }
</script>

{#if shouldVirtualize}
  <div
    class="overflow-y-auto"
    style:max-height={maxHeight}
    onscroll={onScroll}
    use:measureHeight
  >
    <div style="height: {totalHeight}px; position: relative;">
      <div style="transform: translateY({offsetY}px);">
        {#each visibleItems as { item, index } (getKey(item))}
          <div style="height: {rowHeight}px; overflow: hidden;">
            {@render children(item, index)}
          </div>
        {/each}
      </div>
    </div>
  </div>
{:else}
  {#each items as item, i (getKey(item))}
    {@render children(item, i)}
  {/each}
{/if}
