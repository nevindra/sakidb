<script lang="ts">
  import { Search } from '@lucide/svelte';

  export interface ListPickerItem {
    value: string;
    label: string;
    description?: string;
    disabled?: boolean;
  }

  let {
    open = $bindable(false),
    title = '',
    items = [] as ListPickerItem[],
    onselect,
  }: {
    open?: boolean;
    title?: string;
    items?: ListPickerItem[];
    onselect?: (value: string) => void;
  } = $props();

  let query = $state('');
  let selectedIndex = $state(0);
  let inputEl: HTMLInputElement | undefined = $state();
  let listEl: HTMLDivElement | undefined = $state();

  const filtered = $derived.by(() => {
    if (!query) return items;
    const q = query.toLowerCase();
    return items.filter(i => i.label.toLowerCase().includes(q) || i.description?.toLowerCase().includes(q));
  });

  $effect(() => {
    if (open) {
      query = '';
      selectedIndex = 0;
      requestAnimationFrame(() => inputEl?.focus());
    }
  });

  function resetSelection() {
    selectedIndex = 0;
  }

  function scrollSelectedIntoView() {
    requestAnimationFrame(() => {
      if (!listEl) return;
      const selected = listEl.querySelector('[data-selected="true"]');
      selected?.scrollIntoView({ block: 'nearest' });
    });
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      open = false;
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      selectedIndex = Math.min(selectedIndex + 1, filtered.length - 1);
      scrollSelectedIntoView();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      selectedIndex = Math.max(selectedIndex - 1, 0);
      scrollSelectedIntoView();
    } else if (e.key === 'Enter' && filtered.length > 0) {
      e.preventDefault();
      const item = filtered[selectedIndex];
      if (item && !item.disabled) {
        onselect?.(item.value);
        open = false;
      }
    }
  }
</script>

{#if open}
  <!-- Backdrop -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    class="fixed inset-0 z-50 bg-black/50"
    onclick={() => (open = false)}
  >
    <!-- Panel -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div
      class="mx-auto mt-[15vh] w-full max-w-[560px] rounded-lg border border-border/50 bg-popover shadow-2xl overflow-hidden"
      onclick={(e) => e.stopPropagation()}
    >
      <!-- Search input -->
      <div class="flex items-center gap-2 px-3 border-b border-border">
        <Search class="h-3.5 w-3.5 text-muted-foreground shrink-0" />
        <input
          bind:this={inputEl}
          bind:value={query}
          placeholder={title}
          class="flex-1 h-11 bg-transparent text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-0 focus:shadow-none"
          oninput={resetSelection}
          onkeydown={handleKeydown}
        />
      </div>

      <!-- Results -->
      <div bind:this={listEl} class="max-h-[320px] overflow-y-auto py-1">
        {#if filtered.length === 0}
          <div class="px-3 py-6 text-center text-[12px] text-muted-foreground">
            No results
          </div>
        {:else}
          {#each filtered as item, idx (item.value)}
            {@const isSelected = idx === selectedIndex}
            {@const isActionable = !item.disabled}
            <button
              class="w-full flex items-center justify-between px-4 h-9 text-sm transition-colors duration-75
                {isSelected && isActionable ? 'bg-accent text-foreground' : isActionable ? 'text-muted-foreground hover:bg-accent/50 hover:text-foreground' : 'text-muted-foreground/40 cursor-default'}"
              data-selected={isSelected}
              disabled={!isActionable}
              onmouseenter={() => { if (isActionable) selectedIndex = idx; }}
              onclick={() => {
                if (isActionable) {
                  onselect?.(item.value);
                  open = false;
                }
              }}
            >
              <span>{item.label}</span>
              {#if item.description}
                <span class="text-[11px] text-muted-foreground/60">{item.description}</span>
              {/if}
            </button>
          {/each}
        {/if}
      </div>
    </div>
  </div>
{/if}
