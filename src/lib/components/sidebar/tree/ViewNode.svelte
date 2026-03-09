<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { ViewInfo } from '$lib/types';
  import type { FuzzyResult } from '$lib/utils/fuzzy';
  import { Eye } from '@lucide/svelte';
  import * as ContextMenu from '$lib/components/ui/context-menu';
  import { ContextMenuRenderer, viewMenuItems } from '$lib/context-menus';
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
    view: ViewInfo;
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

  async function handleDrop(cascade?: boolean) {
    dropLoading = true;
    try {
      const rid = app.getRuntimeConnectionId(connectionId, databaseName);
      if (!rid || !dialect) return;
      const sql = dialect.dropView(schema, view.name, cascade ?? false);
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
      <Eye class="h-3 w-3 text-sky-400 shrink-0" />
      {#if selfMatch}
        <HighlightMatch name={view.name} positions={selfMatch.positions} />
      {:else}
        <span class="truncate">{view.name}</span>
      {/if}
    </button>
  </ContextMenu.Trigger>
  <ContextMenuRenderer items={viewMenuItems()} ctx={menuCtx} onaction={handleMenuAction} />
</ContextMenu.Root>

<ConfirmDialog
  bind:open={dropConfirmOpen}
  title="Drop View"
  description={`This will permanently drop the view ${schema ? `"${schema}".` : ''}"${view.name}".`}
  confirmLabel="Drop"
  variant="destructive"
  loading={dropLoading}
  {showCascade}
  onconfirm={handleDrop}
/>

<EditViewDialog
  bind:open={editOpen}
  {schema}
  viewName={view.name}
  {connectionId}
  {databaseName}
  onedited={onRefresh}
/>
