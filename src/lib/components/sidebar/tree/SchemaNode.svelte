<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type {
    TableInfo,
    ViewInfo,
    MaterializedViewInfo,
    FunctionInfo,
    SequenceInfo,
    IndexInfo,
    ForeignTableInfo,
  } from '$lib/types';
  import type { FuzzyResult } from '$lib/utils/fuzzy';
  import { Table2, Eye, Layers, FunctionSquare, Hash, ListTree, ExternalLink } from '@lucide/svelte';
  import CategoryFolder from './CategoryFolder.svelte';
  import TableNode from './TableNode.svelte';
  import ViewNode from './ViewNode.svelte';
  import MaterializedViewNode from './MaterializedViewNode.svelte';
  import FunctionNode from './FunctionNode.svelte';
  import ObjectInfoRow from './ObjectInfoRow.svelte';

  let {
    schemaName,
    connectionId,
    databaseName,
    filterQuery = '',
    searchResults = new Map(),
  }: {
    schemaName: string;
    connectionId: string;
    databaseName: string;
    filterQuery?: string;
    searchResults?: Map<string, FuzzyResult>;
  } = $props();

  const app = getAppState();
  const capabilities = $derived(app.getCapabilities(connectionId));
  const schemaPrefix = $derived(`${connectionId}/${databaseName}/${schemaName}`);
  const isSearching = $derived(filterQuery.length > 0);

  // Data caches
  let tables = $state<TableInfo[]>([]);
  let views = $state<ViewInfo[]>([]);
  let materializedViews = $state<MaterializedViewInfo[]>([]);
  let functions = $state<FunctionInfo[]>([]);
  let sequences = $state<SequenceInfo[]>([]);
  let indexes = $state<IndexInfo[]>([]);
  let foreignTables = $state<ForeignTableInfo[]>([]);

  // Partition grouping: map parent table name → child partitions
  const partitionChildren = $derived.by(() => {
    const map = new Map<string, TableInfo[]>();
    for (const t of tables) {
      if (t.is_partition && t.parent_table) {
        const existing = map.get(t.parent_table) ?? [];
        existing.push(t);
        map.set(t.parent_table, existing);
      }
    }
    return map;
  });

  // Display tables = non-partition tables (includes partition parents)
  const displayTables = $derived(tables.filter(t => !t.is_partition));

  // Filtered lists when searching
  const filteredTables = $derived(
    isSearching
      ? displayTables.filter(t => searchResults.has(`${schemaPrefix}/${t.name}`))
      : displayTables
  );
  const filteredViews = $derived(
    isSearching
      ? views.filter(v => searchResults.has(`${schemaPrefix}/${v.name}`))
      : views
  );
  const filteredMatViews = $derived(
    isSearching
      ? materializedViews.filter(v => searchResults.has(`${schemaPrefix}/${v.name}`))
      : materializedViews
  );
  const filteredFunctions = $derived(
    isSearching
      ? functions.filter(f => searchResults.has(`${schemaPrefix}/${f.name}`))
      : functions
  );

  // Helper to check if a category has matching items
  function categoryHasMatch(items: { name: string }[]): boolean {
    return items.some(item => searchResults.has(`${schemaPrefix}/${item.name}`));
  }

  // Tab-to-tree sync: auto-expand Tables when active tab targets a table in this schema
  const revealTablesExpand = $derived(app.selectedObjectPath?.startsWith(schemaPrefix + '/') ?? false);

  // Loaders
  async function loadTables() {
    tables = await app.loadTables(connectionId, databaseName, schemaName);
  }
  async function loadViews() {
    views = await app.loadViews(connectionId, databaseName, schemaName);
  }
  async function loadMaterializedViews() {
    materializedViews = await app.loadMaterializedViews(connectionId, databaseName, schemaName);
  }
  async function loadFunctions() {
    functions = await app.loadFunctions(connectionId, databaseName, schemaName);
  }
  async function loadSequences() {
    sequences = await app.loadSequences(connectionId, databaseName, schemaName);
  }
  async function loadIndexes() {
    indexes = await app.loadIndexes(connectionId, databaseName, schemaName);
  }
  async function loadForeignTables() {
    foreignTables = await app.loadForeignTables(connectionId, databaseName, schemaName);
  }
</script>

<div class="py-px">
  {#if capabilities?.tables !== false}
    <CategoryFolder label="Tables" count={tables.length} icon={Table2} iconClass="text-primary" load={loadTables} autoExpand={isSearching && categoryHasMatch(displayTables)} reveal={revealTablesExpand}>
      {#snippet children()}
        {#each filteredTables as table (table.name)}
          <TableNode {table} schema={schemaName} {connectionId} {databaseName} partitions={partitionChildren.get(table.name)} onRefreshTables={loadTables} {searchResults} {schemaPrefix} />
        {/each}
      {/snippet}
    </CategoryFolder>
  {/if}

  {#if capabilities?.views !== false}
    <CategoryFolder label="Views" count={views.length} icon={Eye} iconClass="text-sky-400" load={loadViews} autoExpand={isSearching && categoryHasMatch(views)}>
      {#snippet children()}
        {#each filteredViews as view (view.name)}
          <ViewNode {view} schema={schemaName} {connectionId} {databaseName} {searchResults} {schemaPrefix} />
        {/each}
      {/snippet}
    </CategoryFolder>
  {/if}

  {#if capabilities?.materialized_views}
    <CategoryFolder label="Materialized Views" count={materializedViews.length} icon={Layers} iconClass="text-violet-400" load={loadMaterializedViews} autoExpand={isSearching && categoryHasMatch(materializedViews)}>
      {#snippet children()}
        {#each filteredMatViews as view (view.name)}
          <MaterializedViewNode {view} schema={schemaName} {connectionId} {databaseName} {searchResults} {schemaPrefix} />
        {/each}
      {/snippet}
    </CategoryFolder>
  {/if}

  {#if capabilities?.functions}
    <CategoryFolder label="Functions" count={functions.length} icon={FunctionSquare} iconClass="text-emerald-400" load={loadFunctions} autoExpand={isSearching && categoryHasMatch(functions)}>
      {#snippet children()}
        {#each filteredFunctions as func (func.name + '(' + func.argument_types + ')')}
          <FunctionNode {func} schema={schemaName} {searchResults} {schemaPrefix} />
        {/each}
      {/snippet}
    </CategoryFolder>
  {/if}

  {#if capabilities?.sequences}
    <CategoryFolder label="Sequences" count={sequences.length} icon={Hash} iconClass="text-orange-400" load={loadSequences}>
      {#snippet children()}
        {#each sequences as seq (seq.name)}
          <ObjectInfoRow item={{ kind: 'sequence', data: seq }} schema={schemaName} {searchResults} {schemaPrefix} />
        {/each}
      {/snippet}
    </CategoryFolder>
  {/if}

  {#if capabilities?.indexes}
    <CategoryFolder label="Indexes" count={indexes.length} icon={ListTree} iconClass="text-teal-400" load={loadIndexes}>
      {#snippet children()}
        {#each indexes as idx (idx.name)}
          <ObjectInfoRow item={{ kind: 'index', data: idx }} schema={schemaName} {searchResults} {schemaPrefix} />
        {/each}
      {/snippet}
    </CategoryFolder>
  {/if}

  {#if capabilities?.foreign_tables}
    <CategoryFolder label="Foreign Tables" count={foreignTables.length} icon={ExternalLink} iconClass="text-rose-400" load={loadForeignTables}>
      {#snippet children()}
        {#each foreignTables as ft (ft.name)}
          <ObjectInfoRow item={{ kind: 'foreign_table', data: ft }} schema={schemaName} {searchResults} {schemaPrefix} />
        {/each}
      {/snippet}
    </CategoryFolder>
  {/if}
</div>
