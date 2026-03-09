<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { ForeignTableInfo } from '$lib/types';
  import type { FuzzyResult } from '$lib/utils/fuzzy';
  import { ExternalLink, ChevronRight, ChevronDown } from '@lucide/svelte';
  import * as ContextMenu from '$lib/components/ui/context-menu';
  import { ContextMenuRenderer, foreignTableMenuItems } from '$lib/context-menus';
  import type { MenuContext } from '$lib/context-menus';
  import ConfirmDialog from '$lib/components/ui/confirm-dialog/ConfirmDialog.svelte';
  import HighlightMatch from '../HighlightMatch.svelte';
  import { getDialect } from '$lib/dialects';
  import { invoke } from '@tauri-apps/api/core';

  let {
    foreignTable,
    schema,
    connectionId,
    databaseName,
    depth = 14,
    searchResults = new Map(),
    schemaPrefix = '',
    onRefresh,
  }: {
    foreignTable: ForeignTableInfo;
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

  const app = getAppState();
  const capabilities = $derived(connectionId ? app.getCapabilities(connectionId) : null);
  const dialect = $derived((() => {
    if (!connectionId) return null;
    const e = app.getSavedConnection(connectionId)?.engine;
    return e ? getDialect(e as import('$lib/types').EngineType) : null;
  })());
  const engineType = $derived(connectionId ? app.getSavedConnection(connectionId)?.engine : null);
  const showCascade = $derived(engineType === 'postgres');

  const selfMatch = $derived(schemaPrefix ? searchResults.get(`${schemaPrefix}/${foreignTable.name}`) : undefined);

  async function handleDrop(cascade?: boolean) {
    dropLoading = true;
    try {
      if (!connectionId || !databaseName) return;
      const rid = app.getRuntimeConnectionId(connectionId, databaseName);
      if (!rid || !dialect) return;
      const sql = dialect.dropForeignTable(schema, foreignTable.name, cascade ?? false);
      await invoke('execute_batch', { activeConnectionId: rid, sql });
      onRefresh?.();
    } catch {
      // Error handled by store
    } finally {
      dropLoading = false;
    }
  }

  const menuCtx: MenuContext = $derived({ capabilities });

  function handleMenuAction(id: string) {
    switch (id) {
      case 'copy-name': return navigator.clipboard.writeText(
        dialect?.qualifiedTable(schema, foreignTable.name) ?? `"${schema}"."${foreignTable.name}"`);
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
      <ExternalLink class="h-3 w-3 text-rose-400 shrink-0" />
      {#if selfMatch}
        <HighlightMatch name={foreignTable.name} positions={selfMatch.positions} />
      {:else}
        <span class="truncate">{foreignTable.name}</span>
      {/if}
      <span class="text-text-dim text-[10px] ml-auto shrink-0">{foreignTable.server_name}</span>
    </button>
  </ContextMenu.Trigger>
  <ContextMenuRenderer items={foreignTableMenuItems()} ctx={menuCtx} onaction={handleMenuAction} />
</ContextMenu.Root>

<ConfirmDialog
  bind:open={dropConfirmOpen}
  title="Drop Foreign Table"
  description={`This will permanently drop the foreign table ${schema ? `"${schema}".` : ''}"${foreignTable.name}".`}
  confirmLabel="Drop"
  variant="destructive"
  loading={dropLoading}
  {showCascade}
  onconfirm={handleDrop}
/>

{#if expanded}
  <div
    class="pr-2 py-0.5 text-xs text-muted-foreground space-y-0.5"
    style:padding-left="{(depth + 3) * 4}px"
  >
    <div><span class="text-text-dim">server:</span> {foreignTable.server_name}</div>
  </div>
{/if}
