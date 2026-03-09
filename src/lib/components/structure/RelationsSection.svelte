<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { StructureTab } from '$lib/types';
  import { Button } from '$lib/components/ui/button';
  import { Plus, Trash2 } from '@lucide/svelte';
  import * as Dialog from '$lib/components/ui/dialog';
  import ConfirmDialog from '$lib/components/ui/confirm-dialog/ConfirmDialog.svelte';
  import DdlPreview from './DdlPreview.svelte';
  import { getDialect } from '$lib/dialects';
  import type { EngineType } from '$lib/types';

  let { tab }: { tab: StructureTab } = $props();

  const app = getAppState();
  const dialect = $derived((() => { const e = app.getSavedConnection(tab.savedConnectionId)?.engine; return e ? getDialect(e as EngineType) : null; })());

  // ── Add FK dialog ──
  let addOpen = $state(false);
  let addName = $state('');
  let addColumns = $state('');
  let addRefSchema = $state('public');
  let addRefTable = $state('');
  let addRefColumns = $state('');
  let addOnUpdate = $state('NO ACTION');
  let addOnDelete = $state('NO ACTION');
  let addLoading = $state(false);

  const addSql = $derived(
    addColumns && addRefTable && addRefColumns
      ? (dialect?.addForeignKey(tab.schema, tab.table, {
          name: addName || undefined,
          columns: addColumns.split(',').map(c => c.trim()).filter(Boolean),
          refSchema: addRefSchema,
          refTable: addRefTable,
          refColumns: addRefColumns.split(',').map(c => c.trim()).filter(Boolean),
          onUpdate: addOnUpdate,
          onDelete: addOnDelete,
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
      addColumns = '';
      addRefTable = '';
      addRefColumns = '';
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

  const refActions = ['NO ACTION', 'RESTRICT', 'CASCADE', 'SET NULL', 'SET DEFAULT'];
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
    <Button variant="outline" size="sm" class="h-7 text-xs" onclick={() => (addOpen = true)}>
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
        <input id="fk-name" class="w-full mt-1 px-2 py-1.5 bg-card border border-border rounded text-sm text-foreground" bind:value={addName} placeholder="fk_table_column" />
      </div>
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="fk-cols">Local Columns (comma separated)</label>
        <input id="fk-cols" class="w-full mt-1 px-2 py-1.5 bg-card border border-border rounded text-sm text-foreground" bind:value={addColumns} placeholder="user_id" />
      </div>
      <div class="grid grid-cols-2 gap-2">
        <div>
          <label class="text-xs font-medium text-muted-foreground" for="fk-ref-schema">Ref Schema</label>
          <input id="fk-ref-schema" class="w-full mt-1 px-2 py-1.5 bg-card border border-border rounded text-sm text-foreground" bind:value={addRefSchema} />
        </div>
        <div>
          <label class="text-xs font-medium text-muted-foreground" for="fk-ref-table">Ref Table</label>
          <input id="fk-ref-table" class="w-full mt-1 px-2 py-1.5 bg-card border border-border rounded text-sm text-foreground" bind:value={addRefTable} placeholder="users" />
        </div>
      </div>
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="fk-ref-cols">Ref Columns (comma separated)</label>
        <input id="fk-ref-cols" class="w-full mt-1 px-2 py-1.5 bg-card border border-border rounded text-sm text-foreground" bind:value={addRefColumns} placeholder="id" />
      </div>
      <div class="grid grid-cols-2 gap-2">
        <div>
          <label class="text-xs font-medium text-muted-foreground" for="fk-on-update">On Update</label>
          <select id="fk-on-update" class="w-full mt-1 px-2 py-1.5 bg-card border border-border rounded text-sm text-foreground" bind:value={addOnUpdate}>
            {#each refActions as action}
              <option value={action}>{action}</option>
            {/each}
          </select>
        </div>
        <div>
          <label class="text-xs font-medium text-muted-foreground" for="fk-on-delete">On Delete</label>
          <select id="fk-on-delete" class="w-full mt-1 px-2 py-1.5 bg-card border border-border rounded text-sm text-foreground" bind:value={addOnDelete}>
            {#each refActions as action}
              <option value={action}>{action}</option>
            {/each}
          </select>
        </div>
      </div>
      <DdlPreview sql={addSql} />
    </div>
    <Dialog.Footer>
      <Button variant="outline" size="sm" onclick={() => (addOpen = false)} disabled={addLoading}>Cancel</Button>
      <Button size="sm" onclick={handleAdd} disabled={!addColumns || !addRefTable || !addRefColumns || addLoading}>Execute</Button>
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
