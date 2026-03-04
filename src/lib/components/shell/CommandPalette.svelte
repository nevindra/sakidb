<script lang="ts">
  import { Search } from '@lucide/svelte';

  type Command = {
    id: string;
    label: string;
    shortcut?: string;
    action: () => void;
  };

  let { commands, open = $bindable(false) }: { commands: Command[]; open: boolean } = $props();

  let query = $state('');
  let selectedIndex = $state(0);
  let inputEl: HTMLInputElement | undefined = $state();

  const filtered = $derived(
    query
      ? commands.filter((c) => c.label.toLowerCase().includes(query.toLowerCase()))
      : commands
  );

  $effect(() => {
    if (open) {
      query = '';
      selectedIndex = 0;
      // Focus input on next tick
      requestAnimationFrame(() => inputEl?.focus());
    }
  });

  // Reset selection when filter changes
  $effect(() => {
    filtered; // track
    selectedIndex = 0;
  });

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      open = false;
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      selectedIndex = Math.min(selectedIndex + 1, filtered.length - 1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      selectedIndex = Math.max(selectedIndex - 1, 0);
    } else if (e.key === 'Enter' && filtered.length > 0) {
      e.preventDefault();
      filtered[selectedIndex].action();
      open = false;
    }
  }
</script>

{#if open}
  <!-- Backdrop -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="fixed inset-0 z-50 bg-black/50"
    onclick={() => (open = false)}
    onkeydown={handleKeydown}
  >
    <!-- Panel -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="mx-auto mt-[15vh] w-full max-w-[560px] rounded-lg border border-border/50 bg-popover shadow-2xl overflow-hidden"
      onclick={(e) => e.stopPropagation()}
      onkeydown={handleKeydown}
    >
      <!-- Search input -->
      <div class="flex items-center gap-2 px-3 border-b border-border">
        <Search class="h-3.5 w-3.5 text-muted-foreground shrink-0" />
        <input
          bind:this={inputEl}
          bind:value={query}
          placeholder="Type a command..."
          class="flex-1 h-11 bg-transparent text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-0 focus:shadow-none"
        />
      </div>

      <!-- Results -->
      <div class="max-h-[320px] overflow-y-auto py-1">
        {#if filtered.length === 0}
          <div class="px-3 py-6 text-center text-[12px] text-muted-foreground">
            No results
          </div>
        {:else}
          {#each filtered as cmd, i (cmd.id)}
            <button
              class="w-full flex items-center justify-between px-4 h-9 text-sm transition-colors duration-75 {i === selectedIndex ? 'bg-accent text-foreground' : 'text-muted-foreground hover:bg-accent/50 hover:text-foreground'}"
              onmouseenter={() => (selectedIndex = i)}
              onclick={() => { cmd.action(); open = false; }}
            >
              <span>{cmd.label}</span>
              {#if cmd.shortcut}
                <span class="text-[11px] text-muted-foreground">{cmd.shortcut}</span>
              {/if}
            </button>
          {/each}
        {/if}
      </div>
    </div>
  </div>
{/if}
