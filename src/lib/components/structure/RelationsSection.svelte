<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { StructureTab, SchemaInfo, TableInfo, ColumnInfo } from '$lib/types';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Plus, Trash2 } from '@lucide/svelte';
  import * as Dialog from '$lib/components/ui/dialog';
  import * as Select from '$lib/components/ui/select';
  import ConfirmDialog from '$lib/components/ui/confirm-dialog/ConfirmDialog.svelte';
  import { MultiSelect } from '$lib/components/ui/multi-select';
  import DdlPreview from './DdlPreview.svelte';
  import { getDialect } from '$lib/dialects';
  import type { EngineType } from '$lib/types';

  let { tab }: { tab: StructureTab } = $props();

  const app = getAppState();
  const dialect = $derived((() => { const e = app.getSavedConnection(tab.savedConnectionId)?.engine; return e ? getDialect(e as EngineType) : null; })());

  const localColumnNames = $derived(tab.columns.map(c => c.name));

  const refActions = ['NO ACTION', 'RESTRICT', 'CASCADE', 'SET NULL', 'SET DEFAULT'];

  // ── Schema / table / column lookups for ref fields ──
  let refSchemas: string[] = $state([]);
  let refTables: string[] = $state([]);
  let refColumnNames: string[] = $state([]);

  function loadRefSchemas() {
    const schemas = app.getSchemas(tab.savedConnectionId, tab.databaseName);
    refSchemas = schemas.map((s: SchemaInfo) => s.name);
  }

  async function loadRefTables(schema: string) {
    refTables = [];
    refColumnNames = [];
    const tables = await app.loadTables(tab.savedConnectionId, tab.databaseName, schema);
    refTables = tables.filter((t: TableInfo) => !t.is_partition).map((t: TableInfo) => t.name);
  }

  async function loadRefColumns(schema: string, table: string) {
    refColumnNames = [];
    const cols = await app.loadColumns(tab.savedConnectionId, tab.databaseName, schema, table);
    refColumnNames = cols.map((c: ColumnInfo) => c.name);
  }

  // ── Add FK dialog ──
  let addOpen = $state(false);
  let addName = $state('');
  let addColumns: string[] = $state([]);
  let addRefSchema = $state('public');
  let addRefTable = $state('');
  let addRefColumns: string[] = $state([]);
  let addOnUpdate = $state('NO ACTION');
  let addOnDelete = $state('NO ACTION');
  let addLoading = $state(false);

  const addSql = $derived(
    addColumns.length > 0 && addRefTable && addRefColumns.length > 0
      ? (dialect?.addForeignKey(tab.schema, tab.table, {
          name: addName || undefined,
          columns: addColumns,
          refSchema: addRefSchema,
          refTable: addRefTable,
          refColumns: addRefColumns,
          onUpdate: addOnUpdate,
          onDelete: addOnDelete,
        }) ?? '')
      : ''
  );

  function handleOpenDialog() {
    addOpen = true;
    loadRefSchemas();
    loadRefTables(addRefSchema);
  }

  function handleRefSchemaChange(schema: string) {
    addRefSchema = schema;
    addRefTable = '';
    addRefColumns = [];
    loadRefTables(schema);
  }

  function handleRefTableChange(table: string) {
    addRefTable = table;
    addRefColumns = [];
    if (table) loadRefColumns(addRefSchema, table);
  }

  async function handleAdd() {
    if (!addSql) return;
    addLoading = true;
    try {
      await app.executeDdl(tab.runtimeConnectionId, addSql);
      addOpen = false;
      addName = '';
      addColumns = [];
      addRefTable = '';
      addRefColumns = [];
      app.loadStructureTab(tab.id);
    } catch {
      // Error shown via toast
    } finally {
      addLoading = false;
    }
  }

  // ── Drop FK ──
  let dropOpen = $state(false);
  let dropName = $state('');

  function confirmDrop(name: string) {
    dropName = name;
    dropOpen = true;
  }

  async function handleDrop() {
    const sql = dialect!.dropConstraint(tab.schema, tab.table, dropName);
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
        <th class="py-1.5 px-2 font-medium">Constraint</th>
        <th class="py-1.5 px-2 font-medium">Columns</th>
        <th class="py-1.5 px-2 font-medium">References</th>
        <th class="py-1.5 px-2 font-medium">On Update</th>
        <th class="py-1.5 px-2 font-medium">On Delete</th>
        <th class="py-1.5 px-2 font-medium w-10"></th>
      </tr>
    </thead>
    <tbody>
      {#each tab.foreignKeys as fk (fk.constraint_name)}
        <tr class="border-b border-border/50 hover:bg-sidebar-accent/50 transition-colors">
          <td class="py-1.5 px-2 font-medium text-foreground">{fk.constraint_name}</td>
          <td class="py-1.5 px-2 text-muted-foreground font-mono">{fk.columns.join(', ')}</td>
          <td class="py-1.5 px-2 text-muted-foreground font-mono">{fk.foreign_table_schema}.{fk.foreign_table_name}({fk.foreign_columns.join(', ')})</td>
          <td class="py-1.5 px-2 text-muted-foreground">{fk.on_update}</td>
          <td class="py-1.5 px-2 text-muted-foreground">{fk.on_delete}</td>
          <td class="py-1.5 px-2">
            <button
              class="p-0.5 text-muted-foreground hover:text-destructive transition-colors"
              onclick={() => confirmDrop(fk.constraint_name)}
            >
              <Trash2 class="h-3 w-3" />
            </button>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>

  {#if tab.foreignKeys.length === 0}
    <p class="text-xs text-muted-foreground py-4 text-center">No foreign keys</p>
  {/if}

  <div class="mt-3">
    <Button variant="outline" size="sm" class="h-7 text-xs" onclick={handleOpenDialog}>
      <Plus class="h-3 w-3 mr-1" />
      Add Foreign Key
    </Button>
  </div>
</div>

<!-- Add FK Dialog -->
<Dialog.Root bind:open={addOpen}>
  <Dialog.Content class="max-w-lg">
    <Dialog.Header>
      <Dialog.Title>Add Foreign Key</Dialog.Title>
    </Dialog.Header>
    <div class="space-y-3 py-2">
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="fk-name">Constraint Name (optional)</label>
        <Input id="fk-name" class="mt-1" bind:value={addName} placeholder="fk_table_column" />
      </div>
      <div>
        <!-- svelte-ignore a11y_label_has_associated_control -->
        <label class="text-xs font-medium text-muted-foreground">Local Columns</label>
        <div class="mt-1">
          <MultiSelect options={localColumnNames} bind:selected={addColumns} placeholder="Select columns..." />
        </div>
      </div>
      <div class="grid grid-cols-2 gap-2">
        <div>
          <!-- svelte-ignore a11y_label_has_associated_control -->
          <label class="text-xs font-medium text-muted-foreground">Ref Schema</label>
          <div class="mt-1">
            <Select.Root type="single" value={addRefSchema} onValueChange={(v) => { if (v) handleRefSchemaChange(v); }}>
              <Select.Trigger class="w-full">
                <span data-slot="select-value">{addRefSchema}</span>
              </Select.Trigger>
              <Select.Content>
                {#each refSchemas as s}
                  <Select.Item value={s} label={s} />
                {/each}
              </Select.Content>
            </Select.Root>
          </div>
        </div>
        <div>
          <!-- svelte-ignore a11y_label_has_associated_control -->
          <label class="text-xs font-medium text-muted-foreground">Ref Table</label>
          <div class="mt-1">
            <Select.Root type="single" value={addRefTable} onValueChange={(v) => { if (v) handleRefTableChange(v); }}>
              <Select.Trigger class="w-full">
                <span data-slot="select-value">{addRefTable || 'Select table...'}</span>
              </Select.Trigger>
              <Select.Content>
                {#each refTables as t}
                  <Select.Item value={t} label={t} />
                {/each}
              </Select.Content>
            </Select.Root>
          </div>
        </div>
      </div>
      <div>
        <!-- svelte-ignore a11y_label_has_associated_control -->
        <label class="text-xs font-medium text-muted-foreground">Ref Columns</label>
        <div class="mt-1">
          <MultiSelect options={refColumnNames} bind:selected={addRefColumns} placeholder="Select ref columns..." />
        </div>
      </div>
      <div class="grid grid-cols-2 gap-2">
        <div>
          <!-- svelte-ignore a11y_label_has_associated_control -->
          <label class="text-xs font-medium text-muted-foreground">On Update</label>
          <div class="mt-1">
            <Select.Root type="single" bind:value={addOnUpdate}>
              <Select.Trigger class="w-full">
                <span data-slot="select-value">{addOnUpdate}</span>
              </Select.Trigger>
              <Select.Content>
                {#each refActions as action}
                  <Select.Item value={action} label={action} />
                {/each}
              </Select.Content>
            </Select.Root>
          </div>
        </div>
        <div>
          <!-- svelte-ignore a11y_label_has_associated_control -->
          <label class="text-xs font-medium text-muted-foreground">On Delete</label>
          <div class="mt-1">
            <Select.Root type="single" bind:value={addOnDelete}>
              <Select.Trigger class="w-full">
                <span data-slot="select-value">{addOnDelete}</span>
              </Select.Trigger>
              <Select.Content>
                {#each refActions as action}
                  <Select.Item value={action} label={action} />
                {/each}
              </Select.Content>
            </Select.Root>
          </div>
        </div>
      </div>
      <DdlPreview sql={addSql} />
    </div>
    <Dialog.Footer>
      <Button variant="outline" size="sm" onclick={() => (addOpen = false)} disabled={addLoading}>Cancel</Button>
      <Button size="sm" onclick={handleAdd} disabled={addColumns.length === 0 || !addRefTable || addRefColumns.length === 0 || addLoading}>Execute</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<!-- Drop FK Confirm -->
<ConfirmDialog
  bind:open={dropOpen}
  title="Drop Foreign Key"
  description={`Are you sure you want to drop constraint "${dropName}"?`}
  confirmLabel="Drop"
  variant="destructive"
  onconfirm={handleDrop}
/>
