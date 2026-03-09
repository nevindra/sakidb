<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { StructureTab } from '$lib/types';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Plus, Pencil, Trash2, Key } from '@lucide/svelte';
  import * as Dialog from '$lib/components/ui/dialog';
  import * as Combobox from '$lib/components/ui/combobox';
  import ConfirmDialog from '$lib/components/ui/confirm-dialog/ConfirmDialog.svelte';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { Switch } from '$lib/components/ui/switch';
  import DdlPreview from './DdlPreview.svelte';
  import { getDialect } from '$lib/dialects';
  import type { EngineType } from '$lib/types';
  import { PG_TYPE_GROUPS, PG_PRECISION_TYPES } from '$lib/dialects/pg-types';
  import { SQLITE_TYPE_GROUPS, SQLITE_PRECISION_TYPES } from '$lib/dialects/sqlite-types';

  let { tab }: { tab: StructureTab } = $props();

  const app = getAppState();
  const engine = $derived(app.getSavedConnection(tab.savedConnectionId)?.engine as EngineType | undefined);
  const dialect = $derived(engine ? getDialect(engine) : null);
  const isPostgres = $derived(engine === 'postgres');

  // ── Add column dialog ──
  let addOpen = $state(false);
  let addName = $state('');
  let addType = $state('text');
  let addTypeSearch = $state('');
  let addPrecision = $state('');
  let addNullable = $state(true);
  let addDefault = $state('');
  let addPrimaryKey = $state(false);
  let addUnique = $state(false);
  let addIsArray = $state(false);
  let addCheck = $state('');
  let addComment = $state('');
  let addLoading = $state(false);

  const typeGroups = $derived(engine === 'sqlite' ? SQLITE_TYPE_GROUPS : PG_TYPE_GROUPS);
  const precisionTypes = $derived(engine === 'sqlite' ? SQLITE_PRECISION_TYPES : PG_PRECISION_TYPES);

  const addPrecisionHint = $derived(precisionTypes[addType] ?? null);

  const addFilteredTypes = $derived(
    addTypeSearch === ''
      ? typeGroups
      : typeGroups
          .map(g => ({ ...g, types: g.types.filter(t => t.includes(addTypeSearch.toLowerCase())) }))
          .filter(g => g.types.length > 0)
  );

  const addSql = $derived(
    addName
      ? (dialect?.addColumn(tab.schema, tab.table, {
          name: addName,
          type: addType,
          nullable: addNullable,
          defaultValue: addDefault || undefined,
          primaryKey: addPrimaryKey,
          unique: addUnique,
          isArray: addIsArray,
          precision: addPrecision || undefined,
          check: addCheck || undefined,
          comment: addComment || undefined,
        }) ?? '')
      : ''
  );

  function resetAddForm() {
    addName = '';
    addType = 'text';
    addTypeSearch = '';
    addPrecision = '';
    addNullable = true;
    addDefault = '';
    addPrimaryKey = false;
    addUnique = false;
    addIsArray = false;
    addCheck = '';
    addComment = '';
  }

  async function handleAdd() {
    if (!addSql) return;
    addLoading = true;
    try {
      await app.executeDdl(tab.runtimeConnectionId, addSql);
      addOpen = false;
      resetAddForm();
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
  let editTypeSearch = $state('');
  let editNullable = $state(true);
  let editDefault = $state('');
  let editLoading = $state(false);
  let editOrigType = $state('');
  let editOrigNullable = $state(true);
  let editOrigDefault = $state('');

  const editFilteredTypes = $derived(
    editTypeSearch === ''
      ? typeGroups
      : typeGroups
          .map(g => ({ ...g, types: g.types.filter(t => t.includes(editTypeSearch.toLowerCase())) }))
          .filter(g => g.types.length > 0)
  );

  const editSql = $derived(
    editOrigName
      ? (dialect?.alterColumn(tab.schema, tab.table, editOrigName, {
          type: editType !== editOrigType ? editType : undefined,
          nullable: editNullable !== editOrigNullable ? editNullable : undefined,
          defaultValue: editDefault !== editOrigDefault
            ? (editDefault || null)
            : undefined,
          rename: editName !== editOrigName ? editName : undefined,
        }) ?? '')
      : ''
  );

  function openEdit(col: { name: string; data_type: string; is_nullable: boolean; default_value: string | null }) {
    editOrigName = col.name;
    editName = col.name;
    editType = col.data_type;
    editOrigType = col.data_type;
    editTypeSearch = '';
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
    const sql = dialect!.dropColumn(tab.schema, tab.table, dropColName);
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
        <Input id="add-col-name" class="mt-1" bind:value={addName} placeholder="column_name" />
      </div>

      <!-- Type + Precision row -->
      <div class="grid gap-2" class:grid-cols-2={addPrecisionHint !== null} class:grid-cols-1={addPrecisionHint === null}>
        <div>
          <label class="text-xs font-medium text-muted-foreground">Type</label>
          <div class="relative mt-1">
            <Combobox.Root type="single" bind:value={addType} onOpenChangeComplete={(o) => { if (!o) addTypeSearch = ''; }}>
              <Combobox.Input
                placeholder="Search types..."
                oninput={(e) => { addTypeSearch = e.currentTarget.value; }}
              />
              <Combobox.Trigger />
              <Combobox.Content>
                {#each addFilteredTypes as group}
                  <Combobox.Group>
                    <Combobox.GroupHeading class="px-2 py-1.5 text-xs font-medium text-muted-foreground">{group.label}</Combobox.GroupHeading>
                    {#each group.types as t}
                      <Combobox.Item value={t} label={t} />
                    {/each}
                  </Combobox.Group>
                {:else}
                  <div class="py-2 text-center text-xs text-muted-foreground">No matching types</div>
                {/each}
              </Combobox.Content>
            </Combobox.Root>
          </div>
        </div>
        {#if addPrecisionHint !== null}
          <div>
            <label class="text-xs font-medium text-muted-foreground" for="add-col-precision">Length / Precision</label>
            <Input id="add-col-precision" class="mt-1" bind:value={addPrecision} placeholder={addPrecisionHint || '...'} />
          </div>
        {/if}
      </div>

      <!-- Constraints row -->
      <div class="flex flex-wrap items-center gap-x-5 gap-y-2">
        <label class="flex items-center gap-2 cursor-pointer">
          <Checkbox bind:checked={addNullable} disabled={addPrimaryKey} />
          <span class="text-xs text-muted-foreground">Nullable</span>
        </label>
        <label class="flex items-center gap-2 cursor-pointer">
          <Checkbox bind:checked={addPrimaryKey} onCheckedChange={(v) => { if (v) { addNullable = false; addUnique = false; } }} />
          <span class="text-xs text-muted-foreground">Primary Key</span>
        </label>
        <label class="flex items-center gap-2 cursor-pointer">
          <Checkbox bind:checked={addUnique} disabled={addPrimaryKey} />
          <span class="text-xs text-muted-foreground">Unique</span>
        </label>
        {#if isPostgres}
          <label class="flex items-center gap-2 cursor-pointer">
            <Switch bind:checked={addIsArray} class="scale-75" />
            <span class="text-xs text-muted-foreground">Array</span>
          </label>
        {/if}
      </div>

      <div>
        <label class="text-xs font-medium text-muted-foreground" for="add-col-default">Default Value</label>
        <Input id="add-col-default" class="mt-1" bind:value={addDefault} placeholder="e.g. 0, 'text', now(), gen_random_uuid()" />
      </div>

      <div>
        <label class="text-xs font-medium text-muted-foreground" for="add-col-check">Check Constraint</label>
        <Input id="add-col-check" class="mt-1" bind:value={addCheck} placeholder="e.g. age > 0, status IN ('active', 'inactive')" />
      </div>

      {#if isPostgres}
        <div>
          <label class="text-xs font-medium text-muted-foreground" for="add-col-comment">Comment</label>
          <Input id="add-col-comment" class="mt-1" bind:value={addComment} placeholder="Column description" />
        </div>
      {/if}

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
        <Input id="edit-col-name" class="mt-1" bind:value={editName} />
      </div>
      <div>
        <label class="text-xs font-medium text-muted-foreground">Type</label>
        <div class="relative mt-1">
          <Combobox.Root type="single" bind:value={editType} onOpenChangeComplete={(o) => { if (!o) editTypeSearch = ''; }}>
            <Combobox.Input
              placeholder="Search types..."
              oninput={(e) => { editTypeSearch = e.currentTarget.value; }}
            />
            <Combobox.Trigger />
            <Combobox.Content>
              {#each editFilteredTypes as group}
                <Combobox.Group>
                  <Combobox.GroupHeading class="px-2 py-1.5 text-xs font-medium text-muted-foreground">{group.label}</Combobox.GroupHeading>
                  {#each group.types as t}
                    <Combobox.Item value={t} label={t} />
                  {/each}
                </Combobox.Group>
              {:else}
                <div class="py-2 text-center text-xs text-muted-foreground">No matching types</div>
              {/each}
            </Combobox.Content>
          </Combobox.Root>
        </div>
      </div>
      <label class="flex items-center gap-2 cursor-pointer">
        <Checkbox bind:checked={editNullable} />
        <span class="text-xs text-muted-foreground">Nullable</span>
      </label>
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="edit-col-default">Default</label>
        <Input id="edit-col-default" class="mt-1" bind:value={editDefault} placeholder="e.g. 0, 'text', now()" />
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
