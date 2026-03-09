<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { StructureTab } from '$lib/types';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Plus, Trash2 } from '@lucide/svelte';
  import * as Dialog from '$lib/components/ui/dialog';
  import * as Select from '$lib/components/ui/select';
  import ConfirmDialog from '$lib/components/ui/confirm-dialog/ConfirmDialog.svelte';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { MultiSelect } from '$lib/components/ui/multi-select';
  import DdlPreview from './DdlPreview.svelte';
  import { getDialect } from '$lib/dialects';
  import type { EngineType } from '$lib/types';
  import { Badge } from '$lib/components/ui/badge';

  let { tab }: { tab: StructureTab } = $props();

  const app = getAppState();
  const dialect = $derived((() => { const e = app.getSavedConnection(tab.savedConnectionId)?.engine; return e ? getDialect(e as EngineType) : null; })());

  const columnNames = $derived(tab.columns.map(c => c.name));

  const indexTypes = ['btree', 'hash', 'gin', 'gist', 'brin', 'spgist'];

  // ── Create index dialog ──
  let addOpen = $state(false);
  let addName = $state('');
  let addColumns: string[] = $state([]);
  let addUnique = $state(false);
  let addType = $state('btree');
  let addLoading = $state(false);

  const addSql = $derived(
    addName && addColumns.length > 0
      ? (dialect?.createIndex(tab.schema, tab.table, {
          name: addName,
          columns: addColumns,
          unique: addUnique,
          type: addType,
        }) ?? '')
      : ''
  );

  async function handleAdd() {
    if (!addSql) return;
    addLoading = true;
    try {
      await app.executeDdl(tab.runtimeConnectionId, addSql);
      addOpen = false;
      addName = '';
      addColumns = [];
      addUnique = false;
      addType = 'btree';
      app.loadStructureTab(tab.id);
    } catch {
      // Error shown via toast
    } finally {
      addLoading = false;
    }
  }

  // ── Drop index ──
  let dropOpen = $state(false);
  let dropIndexName = $state('');

  function confirmDrop(name: string) {
    dropIndexName = name;
    dropOpen = true;
  }

  async function handleDrop() {
    const sql = dialect!.dropIndex(tab.schema, dropIndexName);
    try {
      await app.executeDdl(tab.runtimeConnectionId, sql);
      app.loadStructureTab(tab.id);
    } catch {
      // Error shown via toast
    }
  }
</script>

<div class="p-3">
  <table class="w-full text-xs">
    <thead>
      <tr class="text-left text-muted-foreground border-b border-border">
        <th class="py-1.5 px-2 font-medium">Name</th>
        <th class="py-1.5 px-2 font-medium">Columns</th>
        <th class="py-1.5 px-2 font-medium">Type</th>
        <th class="py-1.5 px-2 font-medium">Properties</th>
        <th class="py-1.5 px-2 font-medium w-10"></th>
      </tr>
    </thead>
    <tbody>
      {#each tab.indexes as idx (idx.name)}
        <tr class="border-b border-border/50 hover:bg-sidebar-accent/50 transition-colors">
          <td class="py-1.5 px-2 font-medium text-foreground">{idx.name}</td>
          <td class="py-1.5 px-2 text-muted-foreground font-mono">{idx.columns}</td>
          <td class="py-1.5 px-2 text-muted-foreground">{idx.index_type}</td>
          <td class="py-1.5 px-2">
            <div class="flex gap-1">
              {#if idx.is_primary}
                <Badge variant="outline" class="text-[10px] py-0 px-1.5 text-warning border-warning/30">PK</Badge>
              {/if}
              {#if idx.is_unique && !idx.is_primary}
                <Badge variant="outline" class="text-[10px] py-0 px-1.5">UNIQUE</Badge>
              {/if}
            </div>
          </td>
          <td class="py-1.5 px-2">
            {#if !idx.is_primary}
              <button
                class="p-0.5 text-muted-foreground hover:text-destructive transition-colors"
                onclick={() => confirmDrop(idx.name)}
              >
                <Trash2 class="h-3 w-3" />
              </button>
            {/if}
          </td>
        </tr>
      {/each}
    </tbody>
  </table>

  {#if tab.indexes.length === 0}
    <p class="text-xs text-muted-foreground py-4 text-center">No indexes</p>
  {/if}

  <div class="mt-3">
    <Button variant="outline" size="sm" class="h-7 text-xs" onclick={() => (addOpen = true)}>
      <Plus class="h-3 w-3 mr-1" />
      Create Index
    </Button>
  </div>
</div>

<!-- Create Index Dialog -->
<Dialog.Root bind:open={addOpen}>
  <Dialog.Content class="max-w-md">
    <Dialog.Header>
      <Dialog.Title>Create Index</Dialog.Title>
    </Dialog.Header>
    <div class="space-y-3 py-2">
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="idx-name">Index Name</label>
        <Input id="idx-name" class="mt-1" bind:value={addName} placeholder="idx_table_column" />
      </div>
      <div>
        <label class="text-xs font-medium text-muted-foreground">Columns</label>
        <div class="mt-1">
          <MultiSelect options={columnNames} bind:selected={addColumns} placeholder="Select columns..." />
        </div>
      </div>
      <div>
        <label class="text-xs font-medium text-muted-foreground">Index Type</label>
        <div class="mt-1">
          <Select.Root type="single" bind:value={addType}>
            <Select.Trigger class="w-full">
              <span data-slot="select-value">{addType}</span>
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
        <Checkbox bind:checked={addUnique} />
        <span class="text-xs text-muted-foreground">Unique</span>
      </label>
      <DdlPreview sql={addSql} />
    </div>
    <Dialog.Footer>
      <Button variant="outline" size="sm" onclick={() => (addOpen = false)} disabled={addLoading}>Cancel</Button>
      <Button size="sm" onclick={handleAdd} disabled={!addName || addColumns.length === 0 || addLoading}>Execute</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<!-- Drop Index Confirm -->
<ConfirmDialog
  bind:open={dropOpen}
  title="Drop Index"
  description={`Are you sure you want to drop index "${dropIndexName}"?`}
  confirmLabel="Drop"
  variant="destructive"
  onconfirm={handleDrop}
/>
