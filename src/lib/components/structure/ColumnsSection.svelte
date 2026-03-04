<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { StructureTab } from '$lib/types';
  import { Button } from '$lib/components/ui/button';
  import { Plus, Pencil, Trash2, Key } from '@lucide/svelte';
  import * as Dialog from '$lib/components/ui/dialog';
  import ConfirmDialog from '$lib/components/ui/confirm-dialog/ConfirmDialog.svelte';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import DdlPreview from './DdlPreview.svelte';
  import { generateAddColumn, generateAlterColumn, generateDropColumn } from '$lib/utils/ddl';

  let { tab }: { tab: StructureTab } = $props();

  const app = getAppState();

  // ── Add column dialog ──
  let addOpen = $state(false);
  let addName = $state('');
  let addType = $state('text');
  let addNullable = $state(true);
  let addDefault = $state('');
  let addLoading = $state(false);

  const addSql = $derived(
    addName
      ? generateAddColumn(tab.schema, tab.table, {
          name: addName,
          type: addType,
          nullable: addNullable,
          defaultValue: addDefault || undefined,
        })
      : ''
  );

  async function handleAdd() {
    if (!addSql) return;
    addLoading = true;
    try {
      await app.executeDdl(tab.runtimeConnectionId, addSql);
      addOpen = false;
      addName = '';
      addType = 'text';
      addNullable = true;
      addDefault = '';
      app.loadStructureTab(tab.id);
    } catch (e) {
      // Error shown via toast
    } finally {
      addLoading = false;
    }
  }

  // ── Edit column dialog ──
  let editOpen = $state(false);
  let editOrigName = $state('');
  let editName = $state('');
  let editType = $state('');
  let editNullable = $state(true);
  let editDefault = $state('');
  let editLoading = $state(false);
  let editOrigType = $state('');
  let editOrigNullable = $state(true);
  let editOrigDefault = $state('');

  const editSql = $derived(
    editOrigName
      ? generateAlterColumn(tab.schema, tab.table, editOrigName, {
          type: editType !== editOrigType ? editType : undefined,
          nullable: editNullable !== editOrigNullable ? editNullable : undefined,
          defaultValue: editDefault !== editOrigDefault
            ? (editDefault || null)
            : undefined,
          rename: editName !== editOrigName ? editName : undefined,
        })
      : ''
  );

  function openEdit(col: { name: string; data_type: string; is_nullable: boolean; default_value: string | null }) {
    editOrigName = col.name;
    editName = col.name;
    editType = col.data_type;
    editOrigType = col.data_type;
    editNullable = col.is_nullable;
    editOrigNullable = col.is_nullable;
    editDefault = col.default_value ?? '';
    editOrigDefault = col.default_value ?? '';
    editOpen = true;
  }

  async function handleEdit() {
    if (!editSql) return;
    editLoading = true;
    try {
      await app.executeDdl(tab.runtimeConnectionId, editSql);
      editOpen = false;
      app.loadStructureTab(tab.id);
    } catch (e) {
      // Error shown via toast
    } finally {
      editLoading = false;
    }
  }

  // ── Drop column ──
  let dropOpen = $state(false);
  let dropColName = $state('');

  function confirmDrop(name: string) {
    dropColName = name;
    dropOpen = true;
  }

  async function handleDrop() {
    const sql = generateDropColumn(tab.schema, tab.table, dropColName);
    try {
      await app.executeDdl(tab.runtimeConnectionId, sql);
      app.loadStructureTab(tab.id);
    } catch (e) {
      // Error shown via toast
    }
  }
</script>

<div class="p-3">
  <table class="w-full text-xs">
    <thead>
      <tr class="text-left text-muted-foreground border-b border-border">
        <th class="py-1.5 px-2 font-medium">Name</th>
        <th class="py-1.5 px-2 font-medium">Type</th>
        <th class="py-1.5 px-2 font-medium">Nullable</th>
        <th class="py-1.5 px-2 font-medium">Default</th>
        <th class="py-1.5 px-2 font-medium">PK</th>
        <th class="py-1.5 px-2 font-medium w-16"></th>
      </tr>
    </thead>
    <tbody>
      {#each tab.columns as col (col.name)}
        <tr class="border-b border-border/50 hover:bg-sidebar-accent/50 transition-colors">
          <td class="py-1.5 px-2 font-medium text-foreground">{col.name}</td>
          <td class="py-1.5 px-2 text-muted-foreground font-mono">{col.data_type}</td>
          <td class="py-1.5 px-2 text-muted-foreground">{col.is_nullable ? 'YES' : 'NO'}</td>
          <td class="py-1.5 px-2 text-muted-foreground font-mono truncate max-w-[200px]">{col.default_value ?? ''}</td>
          <td class="py-1.5 px-2">
            {#if col.is_primary_key}
              <Key class="h-3 w-3 text-warning" />
            {/if}
          </td>
          <td class="py-1.5 px-2">
            <div class="flex items-center gap-1 opacity-0 group-hover:opacity-100" style="opacity: 1">
              <button
                class="p-0.5 text-muted-foreground hover:text-foreground transition-colors"
                onclick={() => openEdit(col)}
              >
                <Pencil class="h-3 w-3" />
              </button>
              <button
                class="p-0.5 text-muted-foreground hover:text-destructive transition-colors"
                onclick={() => confirmDrop(col.name)}
              >
                <Trash2 class="h-3 w-3" />
              </button>
            </div>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>

  <div class="mt-3">
    <Button variant="outline" size="sm" class="h-7 text-xs" onclick={() => (addOpen = true)}>
      <Plus class="h-3 w-3 mr-1" />
      Add Column
    </Button>
  </div>
</div>

<!-- Add Column Dialog -->
<Dialog.Root bind:open={addOpen}>
  <Dialog.Content class="max-w-md">
    <Dialog.Header>
      <Dialog.Title>Add Column</Dialog.Title>
    </Dialog.Header>
    <div class="space-y-3 py-2">
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="add-col-name">Name</label>
        <input id="add-col-name" class="w-full mt-1 px-2 py-1.5 bg-card border border-border rounded text-sm text-foreground" bind:value={addName} />
      </div>
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="add-col-type">Type</label>
        <input id="add-col-type" class="w-full mt-1 px-2 py-1.5 bg-card border border-border rounded text-sm text-foreground" bind:value={addType} />
      </div>
      <label class="flex items-center gap-2 cursor-pointer">
        <Checkbox bind:checked={addNullable} />
        <span class="text-xs text-muted-foreground">Nullable</span>
      </label>
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="add-col-default">Default</label>
        <input id="add-col-default" class="w-full mt-1 px-2 py-1.5 bg-card border border-border rounded text-sm text-foreground" bind:value={addDefault} placeholder="e.g. 0, 'text', now()" />
      </div>
      <DdlPreview sql={addSql} />
    </div>
    <Dialog.Footer>
      <Button variant="outline" size="sm" onclick={() => (addOpen = false)} disabled={addLoading}>Cancel</Button>
      <Button size="sm" onclick={handleAdd} disabled={!addName || addLoading}>Execute</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<!-- Edit Column Dialog -->
<Dialog.Root bind:open={editOpen}>
  <Dialog.Content class="max-w-md">
    <Dialog.Header>
      <Dialog.Title>Edit Column: {editOrigName}</Dialog.Title>
    </Dialog.Header>
    <div class="space-y-3 py-2">
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="edit-col-name">Name</label>
        <input id="edit-col-name" class="w-full mt-1 px-2 py-1.5 bg-card border border-border rounded text-sm text-foreground" bind:value={editName} />
      </div>
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="edit-col-type">Type</label>
        <input id="edit-col-type" class="w-full mt-1 px-2 py-1.5 bg-card border border-border rounded text-sm text-foreground" bind:value={editType} />
      </div>
      <label class="flex items-center gap-2 cursor-pointer">
        <Checkbox bind:checked={editNullable} />
        <span class="text-xs text-muted-foreground">Nullable</span>
      </label>
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="edit-col-default">Default</label>
        <input id="edit-col-default" class="w-full mt-1 px-2 py-1.5 bg-card border border-border rounded text-sm text-foreground" bind:value={editDefault} placeholder="e.g. 0, 'text', now()" />
      </div>
      <DdlPreview sql={editSql} />
    </div>
    <Dialog.Footer>
      <Button variant="outline" size="sm" onclick={() => (editOpen = false)} disabled={editLoading}>Cancel</Button>
      <Button size="sm" onclick={handleEdit} disabled={!editSql || editLoading}>Execute</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<!-- Drop Column Confirm -->
<ConfirmDialog
  bind:open={dropOpen}
  title="Drop Column"
  description={`Are you sure you want to drop column "${dropColName}"? This action cannot be undone.`}
  confirmLabel="Drop"
  variant="destructive"
  onconfirm={handleDrop}
/>
