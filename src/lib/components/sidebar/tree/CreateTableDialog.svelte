<script lang="ts">
  import { getAppState } from '$lib/stores';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Plus, Trash2 } from '@lucide/svelte';
  import * as Dialog from '$lib/components/ui/dialog';
  import * as Combobox from '$lib/components/ui/combobox';
  import DdlPreview from '../../structure/DdlPreview.svelte';
  import { getDialect } from '$lib/dialects';
  import type { EngineType } from '$lib/types';
  import { PG_TYPE_GROUPS, PG_PRECISION_TYPES } from '$lib/dialects/pg-types';
  import { SQLITE_TYPE_GROUPS, SQLITE_PRECISION_TYPES } from '$lib/dialects/sqlite-types';

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
  const isPostgres = $derived(engine === 'postgres');
  const typeGroups = $derived(engine === 'sqlite' ? SQLITE_TYPE_GROUPS : PG_TYPE_GROUPS);
  const precisionTypes = $derived(engine === 'sqlite' ? SQLITE_PRECISION_TYPES : PG_PRECISION_TYPES);

  interface ColumnRow {
    id: number;
    name: string;
    type: string;
    typeSearch: string;
    precision: string;
    nullable: boolean;
    primaryKey: boolean;
    unique: boolean;
    isArray: boolean;
    defaultValue: string;
  }

  let tableName = $state('');
  let columns = $state<ColumnRow[]>([]);
  let nextId = $state(0);
  let loading = $state(false);

  function makeDefaultColumn(): ColumnRow {
    const col: ColumnRow = {
      id: nextId++,
      name: '',
      type: 'text',
      typeSearch: '',
      precision: '',
      nullable: true,
      primaryKey: false,
      unique: false,
      isArray: false,
      defaultValue: '',
    };
    return col;
  }

  function resetForm() {
    tableName = '';
    nextId = 0;
    columns = [makeDefaultColumn()];
  }

  $effect(() => {
    if (open) {
      resetForm();
    }
  });

  function addColumn() {
    columns.push(makeDefaultColumn());
  }

  function removeColumn(id: number) {
    columns = columns.filter(c => c.id !== id);
  }

  function filteredTypesFor(search: string) {
    if (search === '') return typeGroups;
    return typeGroups
      .map(g => ({ ...g, types: g.types.filter(t => t.includes(search.toLowerCase())) }))
      .filter(g => g.types.length > 0);
  }

  const createSql = $derived(
    tableName && columns.some(c => c.name)
      ? (dialect?.createTable(schema, tableName, columns.filter(c => c.name).map(c => ({
          name: c.name,
          type: c.type,
          nullable: c.nullable,
          defaultValue: c.defaultValue || undefined,
          primaryKey: c.primaryKey,
          unique: c.unique,
          isArray: c.isArray,
          precision: c.precision || undefined,
        }))) ?? '')
      : ''
  );

  async function handleCreate() {
    if (!createSql) return;
    loading = true;
    try {
      const rid = app.getRuntimeConnectionId(connectionId, databaseName);
      if (!rid) throw new Error('Not connected');
      await app.executeDdl(rid, createSql);
      open = false;
      resetForm();
      oncreated?.();
    } catch (e) {
      // Error shown via toast
    } finally {
      loading = false;
    }
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="max-w-2xl max-h-[80vh] flex flex-col">
    <Dialog.Header>
      <Dialog.Title>Create Table</Dialog.Title>
    </Dialog.Header>
    <div class="space-y-3 py-2 overflow-y-auto flex-1 min-h-0">
      <!-- Table name -->
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="ct-table-name">Table Name</label>
        <Input id="ct-table-name" class="mt-1" bind:value={tableName} placeholder="table_name" />
      </div>

      <!-- Columns -->
      <div>
        <span class="text-xs font-medium text-muted-foreground">Columns</span>
        <div class="mt-1.5 border border-border rounded-md overflow-hidden">
          <!-- Header -->
          <div class="grid grid-cols-[1fr_1fr_auto_auto] gap-0 bg-muted/50 border-b border-border text-[10px] font-medium text-muted-foreground uppercase tracking-wider">
            <div class="px-2.5 py-1.5">Name</div>
            <div class="px-2.5 py-1.5">Type</div>
            <div class="px-2.5 py-1.5 text-center w-[180px]">Constraints</div>
            <div class="w-8"></div>
          </div>

          <!-- Rows -->
          {#each columns as col, i (col.id)}
            {@const precisionHint = precisionTypes[col.type] ?? null}
            {@const filtered = filteredTypesFor(col.typeSearch)}
            <div class="grid grid-cols-[1fr_1fr_auto_auto] gap-0 items-center group" class:border-b={i < columns.length - 1} class:border-border={i < columns.length - 1}>
              <!-- Name -->
              <div class="px-1.5 py-1">
                <Input bind:value={col.name} placeholder="column_name" class="h-7 text-xs border-0 shadow-none bg-transparent focus-visible:ring-0 px-1" />
              </div>

              <!-- Type -->
              <div class="px-1.5 py-1 flex gap-1 items-center">
                <div class="flex-1">
                  <Combobox.Root type="single" bind:value={col.type} onOpenChangeComplete={(o) => { if (!o) col.typeSearch = ''; }}>
                    <Combobox.Input
                      class="h-7 text-xs border-0 shadow-none bg-transparent focus-visible:ring-0 px-1"
                      placeholder="type..."
                      oninput={(e) => { col.typeSearch = e.currentTarget.value; }}
                    />
                    <Combobox.Trigger class="h-7 w-6" />
                    <Combobox.Content>
                      {#each filtered as group}
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
                {#if precisionHint !== null}
                  <Input bind:value={col.precision} placeholder={precisionHint || '...'} class="h-7 text-xs w-16 border-0 shadow-none bg-muted/50 focus-visible:ring-0 px-1.5 rounded" />
                {/if}
              </div>

              <!-- Constraint toggles -->
              <div class="px-1.5 py-1 flex items-center gap-1 w-[180px] justify-center">
                <button
                  class="px-1.5 py-0.5 rounded text-[10px] font-medium transition-colors {col.primaryKey ? 'bg-warning/15 text-warning border border-warning/30' : 'text-muted-foreground/60 hover:bg-muted border border-transparent hover:border-border'}"
                  onclick={() => { col.primaryKey = !col.primaryKey; if (col.primaryKey) { col.nullable = false; col.unique = false; } }}
                >PK</button>
                <button
                  class="px-1.5 py-0.5 rounded text-[10px] font-medium transition-colors {col.nullable ? 'bg-muted text-muted-foreground border border-border' : 'text-muted-foreground/60 hover:bg-muted border border-transparent hover:border-border'}"
                  onclick={() => { if (!col.primaryKey) col.nullable = !col.nullable; }}
                  disabled={col.primaryKey}
                >NULL</button>
                <button
                  class="px-1.5 py-0.5 rounded text-[10px] font-medium transition-colors {col.unique ? 'bg-primary/15 text-primary border border-primary/30' : 'text-muted-foreground/60 hover:bg-muted border border-transparent hover:border-border'}"
                  onclick={() => { if (!col.primaryKey) col.unique = !col.unique; }}
                  disabled={col.primaryKey}
                >UQ</button>
                {#if isPostgres}
                  <button
                    class="px-1.5 py-0.5 rounded text-[10px] font-medium transition-colors {col.isArray ? 'bg-violet-500/15 text-violet-400 border border-violet-500/30' : 'text-muted-foreground/60 hover:bg-muted border border-transparent hover:border-border'}"
                    onclick={() => { col.isArray = !col.isArray; }}
                  >[]</button>
                {/if}
              </div>

              <!-- Remove -->
              <div class="px-1 py-1 w-8 flex justify-center">
                <button
                  class="p-0.5 text-muted-foreground/40 hover:text-destructive transition-colors opacity-0 group-hover:opacity-100"
                  onclick={() => removeColumn(col.id)}
                  disabled={columns.length <= 1}
                >
                  <Trash2 class="h-3.5 w-3.5" />
                </button>
              </div>
            </div>

            <!-- Default value row (only shown when column has a name) -->
            {#if col.name}
              <div class="px-2.5 pb-1.5 -mt-0.5" class:border-b={i < columns.length - 1} class:border-border={i < columns.length - 1}>
                <Input bind:value={col.defaultValue} placeholder="Default value (e.g. 0, 'text', now(), gen_random_uuid())" class="h-6 text-[11px] border-0 shadow-none bg-muted/30 focus-visible:ring-0 px-2 rounded" />
              </div>
            {/if}
          {/each}

          <!-- Add column row -->
          <button
            class="w-full px-2.5 py-1.5 text-xs text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors flex items-center gap-1.5 border-t border-border"
            onclick={addColumn}
          >
            <Plus class="h-3 w-3" />
            Add column
          </button>
        </div>
      </div>

      <DdlPreview sql={createSql} />
    </div>
    <Dialog.Footer>
      <Button variant="outline" size="sm" onclick={() => (open = false)} disabled={loading}>Cancel</Button>
      <Button size="sm" onclick={handleCreate} disabled={!createSql || loading}>
        {loading ? 'Creating...' : 'Execute'}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
