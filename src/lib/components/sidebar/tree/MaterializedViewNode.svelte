<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { MaterializedViewInfo } from '$lib/types';
  import type { FuzzyResult } from '$lib/utils/fuzzy';
  import { Layers } from '@lucide/svelte';
  import * as ContextMenu from '$lib/components/ui/context-menu';
  import { ContextMenuRenderer, materializedViewMenuItems } from '$lib/context-menus';
  import type { MenuContext } from '$lib/context-menus';
  import ConfirmDialog from '$lib/components/ui/confirm-dialog/ConfirmDialog.svelte';
  import EditViewDialog from './EditViewDialog.svelte';
  import HighlightMatch from '../HighlightMatch.svelte';
  import { getDialect } from '$lib/dialects';
  import { invoke } from '@tauri-apps/api/core';

  let {
    view,
    schema,
    connectionId,
    databaseName,
    depth = 14,
    searchResults = new Map(),
    schemaPrefix = '',
    onRefresh,
  }: {
    view: MaterializedViewInfo;
    schema: string;
    connectionId: string;
    databaseName: string;
    depth?: number;
    searchResults?: Map<string, FuzzyResult>;
    schemaPrefix?: string;
    onRefresh?: () => void;
  } = $props();

  const selfMatch = $derived(schemaPrefix ? searchResults.get(`${schemaPrefix}/${view.name}`) : undefined);

  const app = getAppState();
  const capabilities = $derived(app.getCapabilities(connectionId));
  const dialect = $derived((() => { const e = app.getSavedConnection(connectionId)?.engine; return e ? getDialect(e as import('$lib/types').EngineType) : null; })());
  const engineType = $derived(app.getSavedConnection(connectionId)?.engine);
  const showCascade = $derived(engineType === 'postgres');

  let dropConfirmOpen = $state(false);
  let dropLoading = $state(false);
  let editOpen = $state(false);

  function handleClick() {
    app.openDataTab(connectionId, databaseName, schema, view.name);
  }

  function formatRowCount(count: number | null): string {
    if (count === null) return '';
    if (count >= 1_000_000) return `~${(count / 1_000_000).toFixed(1)}M`;
    if (count >= 1_000) return `~${(count / 1_000).toFixed(1)}k`;
    return `~${count}`;
  }

  async function handleDrop(cascade?: boolean) {
    dropLoading = true;
    try {
      const rid = app.getRuntimeConnectionId(connectionId, databaseName);
      if (!rid || !dialect) return;
      const sql = dialect.dropMaterializedView(schema, view.name, cascade ?? false);
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
      case 'open-data': return handleClick();
      case 'view-structure': return app.openStructureTab(connectionId, databaseName, schema, view.name);
      case 'refresh': {
        const sql = dialect?.refreshMaterializedView(schema, view.name);
        if (sql) app.openQueryTab(connectionId, databaseName, sql);
        return;
      }
      case 'new-query': return app.openQueryTab(connectionId, databaseName,
        `SELECT * FROM ${dialect?.qualifiedTable(schema, view.name) ?? '"' + view.name + '"'} LIMIT 100;`);
      case 'edit': editOpen = true; return;
      case 'copy-name': return navigator.clipboard.writeText(
        dialect?.qualifiedTable(schema, view.name) ?? `"${schema}"."${view.name}"`);
      case 'drop': dropConfirmOpen = true; return;
    }
  }
</script>

<ContextMenu.Root>
  <ContextMenu.Trigger class="block w-full">
    <button
      class="w-full text-left pr-2 py-0.5 text-xs flex items-center gap-1.5 hover:bg-sidebar-accent transition-colors"
      style:padding-left="{depth * 4}px"
      onclick={handleClick}
    >
      <Layers class="h-3 w-3 text-violet-400 shrink-0" />
      {#if selfMatch}
        <HighlightMatch name={view.name} positions={selfMatch.positions} />
      {:else}
        <span class="truncate">{view.name}</span>
      {/if}
      {#if !view.is_populated}
        <span class="text-warning text-[10px] ml-auto shrink-0">unpopulated</span>
      {:else if view.row_count_estimate != null}
        <span class="text-muted-foreground text-[10px] ml-auto shrink-0 tabular-nums">
          {formatRowCount(view.row_count_estimate)}
        </span>
      {/if}
    </button>
  </ContextMenu.Trigger>
  <ContextMenuRenderer items={materializedViewMenuItems()} ctx={menuCtx} onaction={handleMenuAction} />
</ContextMenu.Root>

{#if dropConfirmOpen}
  <ConfirmDialog
    bind:open={dropConfirmOpen}
    title="Drop Materialized View"
    description={`This will permanently drop the materialized view ${schema ? `"${schema}".` : ''}"${view.name}".`}
    confirmLabel="Drop"
    variant="destructive"
    loading={dropLoading}
    {showCascade}
    onconfirm={handleDrop}
  />
{/if}

{#if editOpen}
  <EditViewDialog
    bind:open={editOpen}
    {schema}
    viewName={view.name}
    {connectionId}
    {databaseName}
    materialized
    onedited={onRefresh}
  />
{/if}
