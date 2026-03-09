<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { SequenceInfo } from '$lib/types';
  import type { FuzzyResult } from '$lib/utils/fuzzy';
  import { Hash, ChevronRight, ChevronDown } from '@lucide/svelte';
  import * as ContextMenu from '$lib/components/ui/context-menu';
  import { ContextMenuRenderer, sequenceMenuItems } from '$lib/context-menus';
  import type { MenuContext } from '$lib/context-menus';
  import ConfirmDialog from '$lib/components/ui/confirm-dialog/ConfirmDialog.svelte';
  import EditSequenceDialog from './EditSequenceDialog.svelte';
  import HighlightMatch from '../HighlightMatch.svelte';
  import { getDialect } from '$lib/dialects';
  import { invoke } from '@tauri-apps/api/core';

  let {
    sequence,
    schema,
    connectionId,
    databaseName,
    depth = 14,
    searchResults = new Map(),
    schemaPrefix = '',
    onRefresh,
  }: {
    sequence: SequenceInfo;
    schema: string;
    connectionId?: string;
    databaseName?: string;
    depth?: number;
    searchResults?: Map<string, FuzzyResult>;
    schemaPrefix?: string;
    onRefresh?: () => void;
  } = $props();

  let expanded = $state(false);
  let dropConfirmOpen = $state(false);
  let dropLoading = $state(false);
  let editOpen = $state(false);

  const app = getAppState();
  const capabilities = $derived(connectionId ? app.getCapabilities(connectionId) : null);
  const dialect = $derived((() => {
    if (!connectionId) return null;
    const e = app.getSavedConnection(connectionId)?.engine;
    return e ? getDialect(e as import('$lib/types').EngineType) : null;
  })());
  const engineType = $derived(connectionId ? app.getSavedConnection(connectionId)?.engine : null);
  const showCascade = $derived(engineType === 'postgres');

  const selfMatch = $derived(schemaPrefix ? searchResults.get(`${schemaPrefix}/${sequence.name}`) : undefined);

  async function handleDrop(cascade?: boolean) {
    dropLoading = true;
    try {
      if (!connectionId || !databaseName) return;
      const rid = app.getRuntimeConnectionId(connectionId, databaseName);
      if (!rid || !dialect) return;
      const sql = dialect.dropSequence(schema, sequence.name, cascade ?? false);
      await invoke('execute_batch', { activeConnectionId: rid, sql });
      onRefresh?.();
    } catch {
      // Error handled by store
    } finally {
      dropLoading = false;
    }
  }

  async function handleReset() {
    if (!connectionId || !databaseName || !dialect) return;
    const rid = app.getRuntimeConnectionId(connectionId, databaseName);
    if (!rid) return;
    const sql = dialect.resetSequence(schema, sequence.name);
    if (sql) {
      try {
        await invoke('execute_batch', { activeConnectionId: rid, sql });
        onRefresh?.();
      } catch {
        // Error handled by store
      }
    }
  }

  const menuCtx: MenuContext = $derived({ capabilities });

  function handleMenuAction(id: string) {
    switch (id) {
      case 'edit': editOpen = true; return;
      case 'copy-name': return navigator.clipboard.writeText(
        dialect?.qualifiedTable(schema, sequence.name) ?? `"${schema}"."${sequence.name}"`);
      case 'reset': return handleReset();
      case 'drop': dropConfirmOpen = true; return;
    }
  }
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
      <Hash class="h-3 w-3 text-orange-400 shrink-0" />
      {#if selfMatch}
        <HighlightMatch name={sequence.name} positions={selfMatch.positions} />
      {:else}
        <span class="truncate">{sequence.name}</span>
      {/if}
      <span class="text-text-dim text-[10px] ml-auto shrink-0">{sequence.data_type}</span>
    </button>
  </ContextMenu.Trigger>
  <ContextMenuRenderer items={sequenceMenuItems()} ctx={menuCtx} onaction={handleMenuAction} />
</ContextMenu.Root>

<ConfirmDialog
  bind:open={dropConfirmOpen}
  title="Drop Sequence"
  description={`This will permanently drop the sequence ${schema ? `"${schema}".` : ''}"${sequence.name}".`}
  confirmLabel="Drop"
  variant="destructive"
  loading={dropLoading}
  {showCascade}
  onconfirm={handleDrop}
/>

{#if connectionId && databaseName}
  <EditSequenceDialog
    bind:open={editOpen}
    {schema}
    sequenceName={sequence.name}
    connectionId={connectionId}
    databaseName={databaseName}
    onedited={onRefresh}
  />
{/if}

{#if expanded}
  <div
    class="pr-2 py-0.5 text-xs text-muted-foreground space-y-0.5"
    style:padding-left="{(depth + 3) * 4}px"
  >
    <div><span class="text-text-dim">type:</span> {sequence.data_type}</div>
    {#if sequence.last_value != null}
      <div><span class="text-text-dim">last value:</span> {sequence.last_value}</div>
    {/if}
  </div>
{/if}
