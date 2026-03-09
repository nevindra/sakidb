<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { FunctionInfo } from '$lib/types';
  import type { FuzzyResult } from '$lib/utils/fuzzy';
  import { FunctionSquare, ChevronRight, ChevronDown } from '@lucide/svelte';
  import * as ContextMenu from '$lib/components/ui/context-menu';
  import { ContextMenuRenderer, functionMenuItems } from '$lib/context-menus';
  import type { MenuContext } from '$lib/context-menus';
  import ConfirmDialog from '$lib/components/ui/confirm-dialog/ConfirmDialog.svelte';
  import EditFunctionDialog from './EditFunctionDialog.svelte';
  import HighlightMatch from '../HighlightMatch.svelte';
  import { getDialect } from '$lib/dialects';
  import { invoke } from '@tauri-apps/api/core';

  let {
    func,
    schema,
    connectionId,
    databaseName,
    depth = 14,
    searchResults = new Map(),
    schemaPrefix = '',
    onRefresh,
  }: {
    func: FunctionInfo;
    schema: string;
    connectionId?: string;
    databaseName?: string;
    depth?: number;
    searchResults?: Map<string, FuzzyResult>;
    schemaPrefix?: string;
    onRefresh?: () => void;
  } = $props();

  const selfMatch = $derived(schemaPrefix ? searchResults.get(`${schemaPrefix}/${func.name}`) : undefined);

  const app = getAppState();
  const capabilities = $derived(connectionId ? app.getCapabilities(connectionId) : null);
  const dialect = $derived((() => {
    if (!connectionId) return null;
    const e = app.getSavedConnection(connectionId)?.engine;
    return e ? getDialect(e as import('$lib/types').EngineType) : null;
  })());
  const engineType = $derived(connectionId ? app.getSavedConnection(connectionId)?.engine : null);
  const showCascade = $derived(engineType === 'postgres');

  let expanded = $state(false);
  let dropConfirmOpen = $state(false);
  let dropLoading = $state(false);
  let editOpen = $state(false);

  const displayName = $derived(
    func.argument_types
      ? `${func.name}(${func.argument_types})`
      : `${func.name}()`
  );

  async function handleDrop(cascade?: boolean) {
    dropLoading = true;
    try {
      if (!connectionId || !databaseName) return;
      const rid = app.getRuntimeConnectionId(connectionId, databaseName);
      if (!rid || !dialect) return;
      const sql = dialect.dropFunction(schema, func.name, func.argument_types ?? null, cascade ?? false);
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
      case 'view-structure':
        if (connectionId && databaseName) app.openStructureTab(connectionId, databaseName, schema, func.name);
        return;
      case 'edit': editOpen = true; return;
      case 'copy-name': return navigator.clipboard.writeText(
        dialect?.qualifiedTable(schema, func.name) ?? `"${schema}"."${func.name}"`);
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
      <FunctionSquare class="h-3 w-3 text-emerald-400 shrink-0" />
      {#if selfMatch}
        <HighlightMatch name={func.name} positions={selfMatch.positions} />
      {:else}
        <span class="truncate" title={displayName}>{func.name}</span>
      {/if}
      <span class="text-text-dim text-[10px] ml-auto shrink-0">{func.kind}</span>
    </button>
  </ContextMenu.Trigger>
  <ContextMenuRenderer items={functionMenuItems()} ctx={menuCtx} onaction={handleMenuAction} />
</ContextMenu.Root>

<ConfirmDialog
  bind:open={dropConfirmOpen}
  title="Drop Function"
  description={`This will permanently drop the function "${displayName}".`}
  confirmLabel="Drop"
  variant="destructive"
  loading={dropLoading}
  {showCascade}
  onconfirm={handleDrop}
/>

{#if connectionId && databaseName}
  <EditFunctionDialog
    bind:open={editOpen}
    {schema}
    funcName={func.name}
    funcArgTypes={func.argument_types ?? ''}
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
    {#if func.argument_types}
      <div><span class="text-text-dim">args:</span> {func.argument_types}</div>
    {/if}
    <div><span class="text-text-dim">returns:</span> {func.return_type}</div>
    <div><span class="text-text-dim">lang:</span> {func.language}</div>
  </div>
{/if}
