<script lang="ts">
  import { RotateCcw } from '@lucide/svelte';
  import { commandDefinitions } from '$lib/commands/definitions';
  import {
    getKeybinding,
    setKeybinding,
    resetKeybinding,
    resetAllKeybindings,
    findConflict,
    formatKeybinding,
    normalizeKeybinding,
  } from '$lib/commands';

  let recordingCommandId = $state<string | null>(null);
  let conflictWarning = $state<{ commandId: string; conflictId: string; keybinding: string } | null>(null);
  let searchQuery = $state('');

  const filteredDefinitions = $derived(
    searchQuery
      ? commandDefinitions.filter(d => d.label.toLowerCase().includes(searchQuery.toLowerCase()) || d.id.toLowerCase().includes(searchQuery.toLowerCase()))
      : commandDefinitions
  );

  // Group by category
  const groupedDefinitions = $derived.by(() => {
    const groups = new Map<string, typeof commandDefinitions>();
    for (const def of filteredDefinitions) {
      const list = groups.get(def.category) ?? [];
      list.push(def);
      groups.set(def.category, list);
    }
    return [...groups.entries()];
  });

  function startRecording(commandId: string) {
    conflictWarning = null;
    recordingCommandId = commandId;
  }

  function handleRecordKeydown(e: KeyboardEvent) {
    if (!recordingCommandId) return;
    e.preventDefault();
    e.stopPropagation();

    // Skip modifier-only presses
    if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) return;

    if (e.key === 'Escape') {
      recordingCommandId = null;
      conflictWarning = null;
      return;
    }

    // Build keybinding string
    const parts: string[] = [];
    if (e.altKey) parts.push('Alt');
    if (e.ctrlKey || e.metaKey) parts.push('Ctrl');
    if (e.shiftKey) parts.push('Shift');

    let key = e.key;
    if (key.length === 1) key = key.toUpperCase();
    if (key === ' ') key = 'Space';
    if (key === 'Tab') key = 'Tab';
    parts.push(key);

    const kb = parts.join('+');

    // Check for conflicts
    const conflict = findConflict(kb, recordingCommandId);
    if (conflict) {
      const conflictDef = commandDefinitions.find(d => d.id === conflict);
      conflictWarning = { commandId: recordingCommandId, conflictId: conflict, keybinding: kb };
      return;
    }

    applyKeybinding(recordingCommandId, kb);
  }

  async function applyKeybinding(commandId: string, kb: string) {
    await setKeybinding(commandId, kb);
    recordingCommandId = null;
    conflictWarning = null;
  }

  async function confirmConflictOverride() {
    if (!conflictWarning) return;
    // Remove old binding
    await setKeybinding(conflictWarning.conflictId, null);
    // Set new binding
    await applyKeybinding(conflictWarning.commandId, conflictWarning.keybinding);
  }

  async function handleUnbind(commandId: string) {
    await setKeybinding(commandId, null);
  }

  async function handleReset(commandId: string) {
    await resetKeybinding(commandId);
  }

  async function handleResetAll() {
    await resetAllKeybindings();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="space-y-4" onkeydown={handleRecordKeydown}>
  <!-- Header with search and reset all -->
  <div class="flex items-center gap-3">
    <input
      bind:value={searchQuery}
      placeholder="Search commands..."
      class="flex-1 h-8 px-3 text-sm rounded-md border border-border bg-background text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
    />
    <button
      class="h-8 px-3 text-xs text-muted-foreground hover:text-foreground border border-border rounded-md hover:bg-accent/50 transition-colors"
      onclick={handleResetAll}
    >
      Reset All
    </button>
  </div>

  <!-- Conflict warning banner -->
  {#if conflictWarning}
    {@const conflictDef = commandDefinitions.find(d => d.id === conflictWarning!.conflictId)}
    <div class="p-3 rounded-md border border-yellow-500/30 bg-yellow-500/10 text-sm">
      <p class="text-yellow-200">
        <strong>{formatKeybinding(conflictWarning.keybinding)}</strong> is already bound to <strong>{conflictDef?.label}</strong>.
      </p>
      <div class="flex gap-2 mt-2">
        <button
          class="px-3 py-1 text-xs bg-yellow-600 text-white rounded hover:bg-yellow-500 transition-colors"
          onclick={confirmConflictOverride}
        >
          Override
        </button>
        <button
          class="px-3 py-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
          onclick={() => { conflictWarning = null; recordingCommandId = null; }}
        >
          Cancel
        </button>
      </div>
    </div>
  {/if}

  <!-- Keybinding table grouped by category -->
  {#each groupedDefinitions as [category, defs]}
    <div>
      <h3 class="text-[11px] font-medium text-muted-foreground/70 uppercase tracking-wider mb-2">{category}</h3>
      <div class="border border-border rounded-md overflow-hidden">
        {#each defs as def, i (def.id)}
          {@const kb = getKeybinding(def.id)}
          {@const isRecording = recordingCommandId === def.id}
          {@const isDefault = !kb || kb === def.defaultKeybinding}
          <div
            class="flex items-center justify-between px-3 h-9 text-sm {i > 0 ? 'border-t border-border' : ''} {isRecording ? 'bg-accent/30' : 'hover:bg-accent/20'} transition-colors"
          >
            <span class="text-foreground">{def.label}</span>
            <div class="flex items-center gap-1.5">
              {#if isRecording}
                <span class="text-[11px] text-primary animate-pulse">Press a key combo...</span>
              {:else}
                <button
                  class="min-w-[80px] text-right px-2 py-0.5 rounded text-[11px] font-mono transition-colors
                    {kb ? 'text-muted-foreground bg-muted/50 hover:bg-muted' : 'text-muted-foreground/40 hover:text-muted-foreground hover:bg-muted/30'}"
                  onclick={() => startRecording(def.id)}
                >
                  {kb ? formatKeybinding(kb) : '—'}
                </button>
              {/if}
              {#if kb}
                <button
                  class="p-1 text-muted-foreground/50 hover:text-muted-foreground transition-colors"
                  title="Unbind"
                  onclick={() => handleUnbind(def.id)}
                >
                  ×
                </button>
              {/if}
              {#if !isDefault}
                <button
                  class="p-1 text-muted-foreground/50 hover:text-muted-foreground transition-colors"
                  title="Reset to default"
                  onclick={() => handleReset(def.id)}
                >
                  <RotateCcw class="h-3 w-3" />
                </button>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    </div>
  {/each}
</div>
