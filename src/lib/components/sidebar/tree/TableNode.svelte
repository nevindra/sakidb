<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { TableInfo, ColumnInfo } from '$lib/types';
  import type { FuzzyResult } from '$lib/utils/fuzzy';
  import { ChevronRight, ChevronDown, Table2 } from '@lucide/svelte';
  import * as ContextMenu from '$lib/components/ui/context-menu';
  import ColumnList from './ColumnList.svelte';
  import ExportDialog from '$lib/components/structure/ExportDialog.svelte';
  import RestoreDialog from './RestoreDialog.svelte';
  import ConfirmDialog from '$lib/components/ui/confirm-dialog/ConfirmDialog.svelte';
  import DuplicateTableDialog from './DuplicateTableDialog.svelte';
  import HighlightMatch from '../HighlightMatch.svelte';
  import { invoke } from '@tauri-apps/api/core';
  import TableNode from './TableNode.svelte';

  let {
    table,
    schema,
    connectionId,
    databaseName,
    partitions,
    depth = 14,
    onRefreshTables,
    searchResults = new Map(),
    schemaPrefix = '',
  }: {
    table: TableInfo;
    schema: string;
    connectionId: string;
    databaseName: string;
    partitions?: TableInfo[];
    depth?: number;
    onRefreshTables?: () => void;
    searchResults?: Map<string, FuzzyResult>;
    schemaPrefix?: string;
  } = $props();

  const selfMatch = $derived(schemaPrefix ? searchResults.get(`${schemaPrefix}/${table.name}`) : undefined);

  const app = getAppState();
  const capabilities = $derived(app.getCapabilities(connectionId));

  // Tab-to-tree sync: highlight when active tab targets this table
  const isRevealed = $derived(app.selectedObjectPath === `${schemaPrefix}/${table.name}`);
  const isPartitionParent = $derived((partitions?.length ?? 0) > 0);

  // Column expansion (for regular tables and partition children)
  let columnsExpanded = $state(false);
  let columns = $state<ColumnInfo[]>([]);
  let columnsLoaded = $state(false);

  // Partition expansion (for partition parents)
  let partitionsExpanded = $state(false);

  const expanded = $derived(isPartitionParent ? partitionsExpanded : columnsExpanded);

  async function toggleExpand() {
    if (isPartitionParent) {
      partitionsExpanded = !partitionsExpanded;
    } else {
      if (columnsExpanded) {
        columnsExpanded = false;
        return;
      }
      columnsExpanded = true;
      if (!columnsLoaded) {
        columns = await app.loadColumns(connectionId, databaseName, schema, table.name);
        columnsLoaded = true;
      }
    }
  }

  let exportOpen = $state(false);
  let restoreOpen = $state(false);
  let dropConfirmOpen = $state(false);
  let truncateConfirmOpen = $state(false);
  let duplicateOpen = $state(false);
  let dropLoading = $state(false);
  let truncateLoading = $state(false);

  async function handleDrop() {
    dropLoading = true;
    try {
      const rid = app.getRuntimeConnectionId(connectionId, databaseName);
      if (!rid) return;
      await invoke('execute_batch', {
        activeConnectionId: rid,
        sql: `DROP TABLE "${schema}"."${table.name}" CASCADE;`,
      });
      onRefreshTables?.();
    } catch {
      // Error handled by store
    } finally {
      dropLoading = false;
    }
  }

  async function handleTruncate() {
    truncateLoading = true;
    try {
      const rid = app.getRuntimeConnectionId(connectionId, databaseName);
      if (!rid) return;
      await invoke('execute_batch', {
        activeConnectionId: rid,
        sql: `TRUNCATE TABLE "${schema}"."${table.name}";`,
      });
    } catch {
      // Error handled by store
    } finally {
      truncateLoading = false;
    }
  }

  async function handleCreateSql() {
    try {
      const ddl = await app.getCreateTableSql(connectionId, databaseName, schema, table.name);
      app.openQueryTab(connectionId, databaseName, ddl);
    } catch {
      // Error handled by store
    }
  }

  function handleClick() {
    app.openDataTab(connectionId, databaseName, schema, table.name);
  }

  function formatRowCount(count: number | null): string {
    if (count === null) return '';
    if (count >= 1_000_000) return `~${(count / 1_000_000).toFixed(1)}M`;
    if (count >= 1_000) return `~${(count / 1_000).toFixed(1)}k`;
    return `~${count}`;
  }

  function formatSize(bytes: number | null): string {
    if (bytes === null || bytes < 0) return '';
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} kB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
  }
</script>

<ContextMenu.Root>
  <ContextMenu.Trigger class="block w-full">
    <button
      class="w-full text-left pr-2 py-0.5 text-xs flex items-center gap-1.5 hover:bg-sidebar-accent transition-colors group"
      class:bg-sidebar-accent={isRevealed}
      style:padding-left="{depth * 4}px"
      onclick={handleClick}
    >
      <span
        class="opacity-0 group-hover:opacity-100 transition-opacity cursor-pointer"
        class:opacity-100={expanded}
        role="button"
        tabindex={0}
        onclick={(e: MouseEvent) => { e.stopPropagation(); toggleExpand(); }}
        onkeydown={(e: KeyboardEvent) => { if (e.key === 'Enter') { e.stopPropagation(); toggleExpand(); } }}
      >
        {#if expanded}
          <ChevronDown class="h-3 w-3 text-muted-foreground" />
        {:else}
          <ChevronRight class="h-3 w-3 text-muted-foreground" />
        {/if}
      </span>
      <Table2 class="h-3 w-3 text-primary shrink-0" />
      {#if selfMatch}
        <HighlightMatch name={table.name} positions={selfMatch.positions} />
      {:else}
        <span class="truncate">{table.name}</span>
      {/if}
      <span class="text-muted-foreground text-[10px] ml-auto shrink-0 tabular-nums">
        {#if isPartitionParent}
          {partitions!.length} parts
        {:else}
          {#if table.row_count_estimate != null}{formatRowCount(table.row_count_estimate)}{/if}
          {#if table.row_count_estimate != null && table.size_bytes}
            <span class="text-muted-foreground/50 mx-0.5">·</span>
          {/if}
          {#if table.size_bytes}{formatSize(table.size_bytes)}{/if}
        {/if}
      </span>
    </button>
  </ContextMenu.Trigger>
  <ContextMenu.Content>
    <ContextMenu.Item onclick={handleClick}>Open Data</ContextMenu.Item>
    <ContextMenu.Item onclick={() => app.openStructureTab(connectionId, databaseName, schema, table.name)}>View Structure</ContextMenu.Item>
    {#if capabilities?.introspection !== false}
      <ContextMenu.Item onclick={() => app.openErdTab(connectionId, databaseName, schema, table.name)}>View ERD</ContextMenu.Item>
    {/if}
    {#if capabilities?.sql !== false}
      <ContextMenu.Item onclick={() => app.openQueryTab(connectionId, databaseName, `SELECT * FROM "${schema}"."${table.name}" LIMIT 100;`)}>New Query</ContextMenu.Item>
    {/if}
    <ContextMenu.Separator />
    {#if capabilities?.export !== false}
      <ContextMenu.Item onclick={() => (exportOpen = true)}>Export Table...</ContextMenu.Item>
    {/if}
    {#if capabilities?.restore}
      <ContextMenu.Item onclick={() => (restoreOpen = true)}>Restore from SQL...</ContextMenu.Item>
    {/if}
    {#if capabilities?.sql !== false}
      <ContextMenu.Item onclick={handleCreateSql}>SQL: Create</ContextMenu.Item>
      <ContextMenu.Item onclick={() => (duplicateOpen = true)}>Duplicate Table...</ContextMenu.Item>
      <ContextMenu.Separator />
      <ContextMenu.Item variant="destructive" onclick={() => (truncateConfirmOpen = true)}>Truncate Table...</ContextMenu.Item>
      <ContextMenu.Item variant="destructive" onclick={() => (dropConfirmOpen = true)}>Drop Table...</ContextMenu.Item>
    {/if}
  </ContextMenu.Content>
</ContextMenu.Root>

{#if expanded}
  {#if isPartitionParent}
    {#each partitions! as partition (partition.name)}
      <TableNode table={partition} {schema} {connectionId} {databaseName} depth={depth + 2} {onRefreshTables} />
    {/each}
  {:else}
    <ColumnList {columns} />
  {/if}
{/if}

<ExportDialog
  bind:open={exportOpen}
  savedConnectionId={connectionId}
  {databaseName}
  {schema}
  table={table.name}
/>

<ConfirmDialog
  bind:open={dropConfirmOpen}
  title="Drop Table"
  description={`This will permanently drop "${schema}"."${table.name}". This action cannot be undone.`}
  confirmLabel="Drop"
  variant="destructive"
  loading={dropLoading}
  onconfirm={handleDrop}
/>

<ConfirmDialog
  bind:open={truncateConfirmOpen}
  title="Truncate Table"
  description={`This will delete all rows from "${schema}"."${table.name}".`}
  confirmLabel="Truncate"
  variant="destructive"
  loading={truncateLoading}
  onconfirm={handleTruncate}
/>

<DuplicateTableDialog
  bind:open={duplicateOpen}
  {schema}
  tableName={table.name}
  {connectionId}
  {databaseName}
  onDuplicated={onRefreshTables}
/>

<RestoreDialog
  bind:open={restoreOpen}
  savedConnectionId={connectionId}
  {databaseName}
  {schema}
  table={table.name}
/>
