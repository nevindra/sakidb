<script lang="ts">
  import { untrack } from 'svelte';
  import { getAppState } from '$lib/stores';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Plus, Trash2, ChevronDown, Search, Table } from '@lucide/svelte';
  import * as Dialog from '$lib/components/ui/dialog';
  import * as Popover from '$lib/components/ui/popover';
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
    precision: string;
    nullable: boolean;
    primaryKey: boolean;
    unique: boolean;
    isArray: boolean;
    defaultValue: string;
  }

  let tableName = $state('');
  let columns = $state<ColumnRow[]>([]);
  let loading = $state(false);

  // Popover state managed separately to avoid deep-reactivity cycles
  let typeSearches = $state<Record<number, string>>({});
  let typeOpens = $state<Record<number, boolean>>({});

  // Plain variable — no reactivity needed
  let nextId = 0;

  function makeDefaultColumn(): ColumnRow {
    return {
      id: nextId++,
      name: '',
      type: 'text',
      precision: '',
      nullable: true,
      primaryKey: false,
      unique: false,
      isArray: false,
      defaultValue: '',
    };
  }

  function resetForm() {
    tableName = '';
    nextId = 0;
    columns = [makeDefaultColumn()];
    typeSearches = {};
    typeOpens = {};
  }

  $effect(() => {
    if (open) {
      untrack(() => resetForm());
    }
  });

  function addColumn() {
    columns.push(makeDefaultColumn());
  }

  function removeColumn(id: number) {
    columns = columns.filter(c => c.id !== id);
  }

  function selectType(col: ColumnRow, type: string) {
    col.type = type;
    typeSearches[col.id] = '';
    typeOpens[col.id] = false;
  }

  function filteredTypesFor(search: string) {
    if (!search) return typeGroups;
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
  <Dialog.Content class="max-w-[640px] max-h-[85vh] flex flex-col gap-0 p-0 overflow-hidden">
    <!-- Header -->
    <div class="flex items-center gap-3 px-5 pt-5 pb-4">
      <div class="flex items-center justify-center w-8 h-8 rounded-lg bg-primary/10 text-primary">
        <Table class="w-4 h-4" />
      </div>
      <div>
        <Dialog.Title class="text-sm font-semibold">Create Table</Dialog.Title>
        <p class="text-[11px] text-muted-foreground mt-0.5">{schema} schema</p>
      </div>
    </div>

    <div class="border-t border-border/50"></div>

    <!-- Body -->
    <div class="flex-1 min-h-0 overflow-y-auto px-5 py-4 space-y-4">
      <!-- Table name -->
      <div>
        <label class="text-[11px] font-medium text-muted-foreground uppercase tracking-wider" for="ct-table-name">Table Name</label>
        <Input
          id="ct-table-name"
          class="mt-1.5 h-9 bg-muted/30 border-border/50 focus-visible:border-primary/50 font-mono text-sm"
          bind:value={tableName}
          placeholder="my_table"
        />
      </div>

      <!-- Columns -->
      <div>
        <div class="flex items-center justify-between mb-2">
          <span class="text-[11px] font-medium text-muted-foreground uppercase tracking-wider">Columns</span>
          <span class="text-[11px] text-muted-foreground/60">{columns.length} {columns.length === 1 ? 'column' : 'columns'}</span>
        </div>

        <div class="space-y-2">
          {#each columns as col (col.id)}
            {@const precisionHint = precisionTypes[col.type] ?? null}
            {@const search = typeSearches[col.id] ?? ''}
            {@const filtered = filteredTypesFor(search)}
            <div class="group rounded-lg border border-border/50 bg-muted/20 hover:border-border/80 transition-colors">
              <!-- Main row -->
              <div class="flex items-center gap-2 px-3 py-2">
                <!-- Column name -->
                <div class="flex-1 min-w-0">
                  <input
                    bind:value={col.name}
                    placeholder="column_name"
                    class="w-full h-7 bg-transparent text-sm font-mono placeholder:text-muted-foreground/40 outline-none"
                  />
                </div>

                <!-- Type picker -->
                <Popover.Root
                  open={typeOpens[col.id] ?? false}
                  onOpenChange={(o) => { typeOpens[col.id] = o; if (!o) typeSearches[col.id] = ''; }}
                >
                  <Popover.Trigger
                    class="flex items-center gap-1.5 h-7 px-2.5 rounded-md bg-muted/60 border border-border/40 hover:border-border/80 hover:bg-muted text-xs font-mono text-muted-foreground transition-colors whitespace-nowrap cursor-pointer"
                  >
                    <span class={col.type ? 'text-foreground' : ''}>{col.type || 'type'}</span>
                    {#if precisionHint !== null && col.precision}
                      <span class="text-muted-foreground/60">({col.precision})</span>
                    {/if}
                    <ChevronDown class="w-3 h-3 opacity-50 shrink-0" />
                  </Popover.Trigger>
                  <Popover.Content align="start" class="w-56 p-0" sideOffset={6}>
                    <!-- Search -->
                    <div class="flex items-center gap-2 px-3 py-2 border-b border-border/50">
                      <Search class="w-3.5 h-3.5 text-muted-foreground/50 shrink-0" />
                      <input
                        value={search}
                        oninput={(e) => { typeSearches[col.id] = e.currentTarget.value; }}
                        placeholder="Search types..."
                        class="flex-1 bg-transparent text-xs outline-none placeholder:text-muted-foreground/40"
                      />
                    </div>
                    <!-- Type list -->
                    <div class="max-h-[240px] overflow-y-auto py-1">
                      {#each filtered as group}
                        <div class="px-3 py-1.5 text-[10px] font-medium text-muted-foreground/50 uppercase tracking-wider sticky top-0 bg-popover">{group.label}</div>
                        {#each group.types as t}
                          <button
                            class="w-full text-left px-3 py-1.5 text-xs font-mono hover:bg-accent/50 transition-colors {col.type === t ? 'text-primary bg-primary/5' : 'text-foreground'}"
                            onclick={() => selectType(col, t)}
                          >
                            {t}
                          </button>
                        {/each}
                      {:else}
                        <div class="px-3 py-4 text-center text-xs text-muted-foreground/50">No matching types</div>
                      {/each}
                    </div>
                  </Popover.Content>
                </Popover.Root>

                <!-- Precision input (conditional) -->
                {#if precisionHint !== null}
                  <input
                    bind:value={col.precision}
                    placeholder={precisionHint || '...'}
                    class="w-14 h-7 px-2 rounded-md bg-muted/60 border border-border/40 text-xs font-mono text-center placeholder:text-muted-foreground/40 outline-none focus:border-border/80"
                  />
                {/if}

                <!-- Constraint toggles -->
                <div class="flex items-center gap-1 ml-1">
                  <button
                    type="button"
                    class="h-6 px-1.5 rounded text-[10px] font-semibold tracking-wide transition-all duration-150 {col.primaryKey
                      ? 'bg-amber-500/15 text-amber-400 ring-1 ring-amber-500/25'
                      : 'text-muted-foreground/40 hover:text-muted-foreground/70 hover:bg-muted/80'}"
                    onclick={() => { col.primaryKey = !col.primaryKey; if (col.primaryKey) { col.nullable = false; col.unique = false; } }}
                    title="Primary Key"
                  >PK</button>
                  <button
                    type="button"
                    class="h-6 px-1.5 rounded text-[10px] font-semibold tracking-wide transition-all duration-150 {col.nullable && !col.primaryKey
                      ? 'bg-muted text-muted-foreground ring-1 ring-border/50'
                      : 'text-muted-foreground/40 hover:text-muted-foreground/70 hover:bg-muted/80'}"
                    onclick={() => { if (!col.primaryKey) col.nullable = !col.nullable; }}
                    disabled={col.primaryKey}
                    title="Nullable"
                  >NULL</button>
                  <button
                    type="button"
                    class="h-6 px-1.5 rounded text-[10px] font-semibold tracking-wide transition-all duration-150 {col.unique
                      ? 'bg-primary/15 text-primary ring-1 ring-primary/25'
                      : 'text-muted-foreground/40 hover:text-muted-foreground/70 hover:bg-muted/80'}"
                    onclick={() => { if (!col.primaryKey) col.unique = !col.unique; }}
                    disabled={col.primaryKey}
                    title="Unique"
                  >UQ</button>
                  {#if isPostgres}
                    <button
                      type="button"
                      class="h-6 px-1.5 rounded text-[10px] font-semibold tracking-wide transition-all duration-150 {col.isArray
                        ? 'bg-violet-500/15 text-violet-400 ring-1 ring-violet-500/25'
                        : 'text-muted-foreground/40 hover:text-muted-foreground/70 hover:bg-muted/80'}"
                      onclick={() => { col.isArray = !col.isArray; }}
                      title="Array type"
                    >[]</button>
                  {/if}
                </div>

                <!-- Remove -->
                <button
                  type="button"
                  class="w-6 h-6 flex items-center justify-center text-muted-foreground/30 hover:text-destructive rounded transition-colors opacity-0 group-hover:opacity-100"
                  onclick={() => removeColumn(col.id)}
                  disabled={columns.length <= 1}
                  title="Remove column"
                >
                  <Trash2 class="w-3.5 h-3.5" />
                </button>
              </div>

              <!-- Default value (shown when column has a name) -->
              {#if col.name}
                <div class="px-3 pb-2 -mt-0.5">
                  <input
                    bind:value={col.defaultValue}
                    placeholder="Default value (e.g. 0, 'text', now(), gen_random_uuid())"
                    class="w-full h-6 px-2 rounded bg-muted/40 text-[11px] font-mono placeholder:text-muted-foreground/30 outline-none focus:bg-muted/60 transition-colors"
                  />
                </div>
              {/if}
            </div>
          {/each}
        </div>

        <!-- Add column -->
        <button
          type="button"
          class="w-full mt-2 flex items-center justify-center gap-1.5 h-8 rounded-lg border border-dashed border-border/50 text-xs text-muted-foreground/60 hover:text-muted-foreground hover:border-border/80 hover:bg-muted/20 transition-colors"
          onclick={addColumn}
        >
          <Plus class="w-3.5 h-3.5" />
          Add column
        </button>
      </div>

      <DdlPreview sql={createSql} />
    </div>

    <!-- Footer -->
    <div class="border-t border-border/50"></div>
    <div class="flex items-center justify-end gap-2 px-5 py-3">
      <Button variant="outline" size="sm" onclick={() => (open = false)} disabled={loading}>Cancel</Button>
      <Button size="sm" onclick={handleCreate} disabled={!createSql || loading}>
        {loading ? 'Creating...' : 'Create Table'}
      </Button>
    </div>
  </Dialog.Content>
</Dialog.Root>
