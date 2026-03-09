<script lang="ts">
  import { getAppState } from '$lib/stores';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import * as Dialog from '$lib/components/ui/dialog';
  import * as Select from '$lib/components/ui/select';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { MultiSelect } from '$lib/components/ui/multi-select';
  import DdlPreview from '../../structure/DdlPreview.svelte';
  import { getDialect } from '$lib/dialects';
  import { invoke } from '@tauri-apps/api/core';
  import type { EngineType, ColumnInfo, TableInfo } from '$lib/types';

  let {
    open = $bindable(false),
    schema,
    connectionId,
    databaseName,
    oncreated,
  }: {
    open?: boolean;
    schema: string;
    connectionId: string;
    databaseName: string;
    oncreated?: () => void;
  } = $props();

  const app = getAppState();
  const engine = $derived(app.getSavedConnection(connectionId)?.engine as EngineType | undefined);
  const dialect = $derived(engine ? getDialect(engine) : null);

  const indexTypes = ['btree', 'hash', 'gin', 'gist', 'brin', 'spgist'];

  let name = $state('');
  let selectedTable = $state('');
  let tables: string[] = $state([]);
  let tableColumns: string[] = $state([]);
  let addColumns: string[] = $state([]);
  let indexType = $state('btree');
  let unique = $state(false);
  let loading = $state(false);

  $effect(() => {
    if (open) {
      resetForm();
      loadTables();
    }
  });

  async function loadTables() {
    const rid = app.getRuntimeConnectionId(connectionId, databaseName);
    if (!rid) return;
    try {
      const result = await invoke<TableInfo[]>('list_tables', {
        activeConnectionId: rid,
        schema: schema,
      });
      tables = result.filter(t => !t.is_partition).map(t => t.name);
    } catch {
      tables = [];
    }
  }

  async function onTableChange(tableName: string) {
    selectedTable = tableName;
    addColumns = [];
    const rid = app.getRuntimeConnectionId(connectionId, databaseName);
    if (!rid) return;
    try {
      const cols = await invoke<ColumnInfo[]>('list_columns', {
        activeConnectionId: rid,
        schema: schema,
        table: tableName,
      });
      tableColumns = cols.map(c => c.name);
    } catch {
      tableColumns = [];
    }
  }

  const createSql = $derived(
    name && selectedTable && addColumns.length > 0
      ? (dialect?.createIndex(schema, selectedTable, {
          name,
          columns: addColumns,
          unique,
          type: indexType,
        }) ?? '')
      : ''
  );

  function resetForm() {
    name = '';
    selectedTable = '';
    tables = [];
    tableColumns = [];
    addColumns = [];
    indexType = 'btree';
    unique = false;
  }

  async function handleCreate() {
    if (!createSql) return;
    loading = true;
    try {
      const rid = app.getRuntimeConnectionId(connectionId, databaseName);
      if (!rid) return;
      await app.executeDdl(rid, createSql);
      open = false;
      resetForm();
      oncreated?.();
    } catch {
      // Error handled by store
    } finally {
      loading = false;
    }
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="max-w-md">
    <Dialog.Header>
      <Dialog.Title>Create Index</Dialog.Title>
    </Dialog.Header>
    <div class="space-y-3 py-2">
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="idx-name">Index Name</label>
        <Input id="idx-name" class="mt-1" bind:value={name} placeholder="idx_table_column" />
      </div>
      <div>
        <label class="text-xs font-medium text-muted-foreground">Table</label>
        <div class="mt-1">
          <Select.Root type="single" value={selectedTable} onValueChange={onTableChange}>
            <Select.Trigger class="w-full">
              <span data-slot="select-value">{selectedTable || 'Select a table...'}</span>
            </Select.Trigger>
            <Select.Content>
              {#each tables as t}
                <Select.Item value={t} label={t} />
              {/each}
            </Select.Content>
          </Select.Root>
        </div>
      </div>
      <div>
        <label class="text-xs font-medium text-muted-foreground">Columns</label>
        <div class="mt-1">
          <MultiSelect options={tableColumns} bind:selected={addColumns} placeholder="Select columns..." />
        </div>
      </div>
      <div>
        <label class="text-xs font-medium text-muted-foreground">Index Type</label>
        <div class="mt-1">
          <Select.Root type="single" bind:value={indexType}>
            <Select.Trigger class="w-full">
              <span data-slot="select-value">{indexType}</span>
            </Select.Trigger>
            <Select.Content>
              {#each indexTypes as t}
                <Select.Item value={t} label={t} />
              {/each}
            </Select.Content>
          </Select.Root>
        </div>
      </div>
      <label class="flex items-center gap-2 cursor-pointer">
        <Checkbox bind:checked={unique} />
        <span class="text-xs text-muted-foreground">Unique</span>
      </label>
      <DdlPreview sql={createSql} />
    </div>
    <Dialog.Footer>
      <Button variant="outline" size="sm" onclick={() => (open = false)} disabled={loading}>Cancel</Button>
      <Button size="sm" onclick={handleCreate} disabled={!name || !selectedTable || addColumns.length === 0 || loading}>Execute</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
