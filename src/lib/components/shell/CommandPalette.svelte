<script lang="ts">
  import { Search } from '@lucide/svelte';
  import {
    getCommands,
    getRecentCommands,
    getKeybinding,
    executeCommand,
    fuzzyMatch,
    formatKeybinding,
  } from '$lib/commands';
  import type { Command, CommandContext } from '$lib/commands';

  const contextLabels: Record<CommandContext, string> = {
    'global': '',
    'connected': 'needs connection',
    'query-tab': 'query tab',
    'data-tab': 'data tab',
    'structure-tab': 'structure tab',
    'erd-tab': 'ERD tab',
    'sidebar': 'sidebar',
  };

  function contextLabel(contexts: CommandContext[]): string {
    const labels = contexts.filter(c => c !== 'global').map(c => contextLabels[c]);
    return labels.join(' / ') || 'unavailable';
  }

  let { open = $bindable(false) }: { open: boolean } = $props();

  let query = $state('');
  let selectedIndex = $state(0);
  let inputEl: HTMLInputElement | undefined = $state();
  let listEl: HTMLDivElement | undefined = $state();

  // Build categorized, filtered command list
  const filteredEntries = $derived.by(() => {
    const commands = getCommands();

    if (!query) {
      // Show recent first, then all by category
      const recent = getRecentCommands();
      const recentIds = new Set(recent.map(c => c.id));
      const groups: { category: string; commands: Command[] }[] = [];

      if (recent.length > 0) {
        groups.push({ category: 'Recent', commands: recent });
      }

      // Group remaining by category
      const byCategory = new Map<string, Command[]>();
      for (const cmd of commands) {
        if (recentIds.has(cmd.id)) continue;
        const list = byCategory.get(cmd.category) ?? [];
        list.push(cmd);
        byCategory.set(cmd.category, list);
      }
      for (const [category, cmds] of byCategory) {
        groups.push({ category, commands: cmds });
      }

      return groups;
    }

    // Fuzzy filter and sort
    const scored = commands
      .map(cmd => ({ cmd, ...fuzzyMatch(query, cmd.label) }))
      .filter(s => s.match)
      .sort((a, b) => {
        // Enabled commands first
        if (a.cmd.enabled !== b.cmd.enabled) return a.cmd.enabled ? -1 : 1;
        return b.score - a.score;
      });

    if (scored.length === 0) return [];
    return [{ category: '', commands: scored.map(s => s.cmd) }];
  });

  // Flat list of selectable commands for keyboard navigation
  const flatCommands = $derived(filteredEntries.flatMap(g => g.commands));

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
      selectedIndex = Math.min(selectedIndex + 1, flatCommands.length - 1);
      scrollSelectedIntoView();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      selectedIndex = Math.max(selectedIndex - 1, 0);
      scrollSelectedIntoView();
    } else if (e.key === 'Enter' && flatCommands.length > 0) {
      e.preventDefault();
      const cmd = flatCommands[selectedIndex];
      if (cmd?.enabled) {
        executeCommand(cmd.id);
        open = false;
      }
    }
  }
</script>

{#if open}
  <!-- Backdrop -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="fixed inset-0 z-50 bg-black/50"
    onclick={() => (open = false)}
  >
    <!-- Panel -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
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
          placeholder="Type a command..."
          class="flex-1 h-11 bg-transparent text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-0 focus:shadow-none"
          oninput={resetSelection}
          onkeydown={handleKeydown}
        />
      </div>

      <!-- Results -->
      <div bind:this={listEl} class="max-h-[320px] overflow-y-auto py-1">
        {#if flatCommands.length === 0}
          <div class="px-3 py-6 text-center text-[12px] text-muted-foreground">
            No results
          </div>
        {:else}
          {#each filteredEntries as group}
            {#if group.category}
              <div class="px-3 pt-2 pb-1 text-[11px] font-medium text-muted-foreground/70 uppercase tracking-wider">
                {group.category}
              </div>
            {/if}
            {#each group.commands as cmd (cmd.id)}
              {@const idx = flatCommands.indexOf(cmd)}
              {@const isSelected = idx === selectedIndex}
              {@const kb = getKeybinding(cmd.id)}
              {@const isActionable = cmd.enabled && !cmd.comingSoon}
              <button
                class="w-full flex items-center justify-between px-4 h-9 text-sm transition-colors duration-75
                  {isSelected && isActionable ? 'bg-accent text-foreground' : isActionable ? 'text-muted-foreground hover:bg-accent/50 hover:text-foreground' : 'text-muted-foreground/40 cursor-default'}"
                data-selected={isSelected}
                disabled={!isActionable}
                onmouseenter={() => { if (isActionable) selectedIndex = idx; }}
                onclick={() => {
                  if (isActionable) {
                    executeCommand(cmd.id);
                    open = false;
                  }
                }}
              >
                <span class={cmd.enabled && !cmd.comingSoon ? '' : 'text-muted-foreground/40'}>{cmd.label}</span>
                <div class="flex items-center gap-2">
                  {#if cmd.comingSoon}
                    <span class="text-[10px] text-muted-foreground/30 italic">coming soon</span>
                  {:else if !cmd.enabled}
                    <span class="text-[10px] text-muted-foreground/30 italic">{contextLabel(cmd.contexts)}</span>
                  {/if}
                  {#if kb}
                    <kbd class="text-[11px] text-muted-foreground font-mono px-1.5 py-0.5 rounded bg-muted/50">{formatKeybinding(kb)}</kbd>
                  {/if}
                </div>
              </button>
            {/each}
          {/each}
        {/if}
      </div>
    </div>
  </div>
{/if}
