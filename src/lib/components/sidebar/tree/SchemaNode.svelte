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
  import VirtualTreeList from './VirtualTreeList.svelte';
  import TableNode from './TableNode.svelte';
  import ViewNode from './ViewNode.svelte';
  import MaterializedViewNode from './MaterializedViewNode.svelte';
  import FunctionNode from './FunctionNode.svelte';
  import SequenceNode from './SequenceNode.svelte';
  import IndexNode from './IndexNode.svelte';
  import ForeignTableNode from './ForeignTableNode.svelte';
  import {
    tablesFolderMenuItems,
    viewsFolderMenuItems,
    materializedViewsFolderMenuItems,
    functionsFolderMenuItems,
    sequencesFolderMenuItems,
    indexesFolderMenuItems,
  } from '$lib/context-menus';
  import type { MenuContext } from '$lib/context-menus';
  import { getDialect } from '$lib/dialects';
  import CreateTableDialog from './CreateTableDialog.svelte';
  import CreateViewDialog from './CreateViewDialog.svelte';
  import CreateFunctionDialog from './CreateFunctionDialog.svelte';
  import CreateSequenceDialog from './CreateSequenceDialog.svelte';
  import CreateIndexDialog from './CreateIndexDialog.svelte';

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

  const dialect = $derived((() => {
    const e = app.getSavedConnection(connectionId)?.engine;
    return e ? getDialect(e as import('$lib/types').EngineType) : null;
  })());

  const folderMenuCtx: MenuContext = $derived({ capabilities });

  // Create dialog state
  let showCreateTable = $state(false);
  let showCreateView = $state(false);
  let showCreateMatView = $state(false);
  let showCreateFunction = $state(false);
  let showCreateSequence = $state(false);
  let showCreateIndex = $state(false);

  function handleFolderCreate(objectType: 'table' | 'view' | 'materialized_view' | 'function' | 'sequence' | 'index') {
    switch (objectType) {
      case 'table': showCreateTable = true; return;
      case 'view': showCreateView = true; return;
      case 'materialized_view': showCreateMatView = true; return;
      case 'function': showCreateFunction = true; return;
      case 'sequence': showCreateSequence = true; return;
      case 'index': showCreateIndex = true; return;
    }
  }
</script>

<div class="py-px">
  {#if capabilities?.tables !== false}
    <CategoryFolder label="Tables" count={tables.length} icon={Table2} iconClass="text-primary" load={loadTables} autoExpand={isSearching && categoryHasMatch(displayTables)} reveal={revealTablesExpand} menuItems={tablesFolderMenuItems()} menuCtx={folderMenuCtx} onmenuaction={() => handleFolderCreate('table')}>
      {#snippet children()}
        <VirtualTreeList items={filteredTables} getKey={(t) => t.name}>
          {#snippet children(table)}
            <TableNode {table} schema={schemaName} {connectionId} {databaseName} partitions={partitionChildren.get(table.name)} onRefreshTables={loadTables} {searchResults} {schemaPrefix} />
          {/snippet}
        </VirtualTreeList>
      {/snippet}
    </CategoryFolder>
  {/if}

  {#if capabilities?.views !== false}
    <CategoryFolder label="Views" count={views.length} icon={Eye} iconClass="text-sky-400" load={loadViews} autoExpand={isSearching && categoryHasMatch(views)} menuItems={viewsFolderMenuItems()} menuCtx={folderMenuCtx} onmenuaction={() => handleFolderCreate('view')}>
      {#snippet children()}
        <VirtualTreeList items={filteredViews} getKey={(v) => v.name}>
          {#snippet children(view)}
            <ViewNode {view} schema={schemaName} {connectionId} {databaseName} {searchResults} {schemaPrefix} onRefresh={loadViews} />
          {/snippet}
        </VirtualTreeList>
      {/snippet}
    </CategoryFolder>
  {/if}

  {#if capabilities?.materialized_views}
    <CategoryFolder label="Materialized Views" count={materializedViews.length} icon={Layers} iconClass="text-violet-400" load={loadMaterializedViews} autoExpand={isSearching && categoryHasMatch(materializedViews)} menuItems={materializedViewsFolderMenuItems()} menuCtx={folderMenuCtx} onmenuaction={() => handleFolderCreate('materialized_view')}>
      {#snippet children()}
        <VirtualTreeList items={filteredMatViews} getKey={(v) => v.name}>
          {#snippet children(view)}
            <MaterializedViewNode {view} schema={schemaName} {connectionId} {databaseName} {searchResults} {schemaPrefix} onRefresh={loadMaterializedViews} />
          {/snippet}
        </VirtualTreeList>
      {/snippet}
    </CategoryFolder>
  {/if}

  {#if capabilities?.functions}
    <CategoryFolder label="Functions" count={functions.length} icon={FunctionSquare} iconClass="text-emerald-400" load={loadFunctions} autoExpand={isSearching && categoryHasMatch(functions)} menuItems={functionsFolderMenuItems()} menuCtx={folderMenuCtx} onmenuaction={() => handleFolderCreate('function')}>
      {#snippet children()}
        <VirtualTreeList items={filteredFunctions} getKey={(f) => f.name + '(' + f.argument_types + ')'}>
          {#snippet children(func)}
            <FunctionNode {func} schema={schemaName} {connectionId} {databaseName} {searchResults} {schemaPrefix} onRefresh={loadFunctions} />
          {/snippet}
        </VirtualTreeList>
      {/snippet}
    </CategoryFolder>
  {/if}

  {#if capabilities?.sequences}
    <CategoryFolder label="Sequences" count={sequences.length} icon={Hash} iconClass="text-orange-400" load={loadSequences} menuItems={sequencesFolderMenuItems()} menuCtx={folderMenuCtx} onmenuaction={() => handleFolderCreate('sequence')}>
      {#snippet children()}
        <VirtualTreeList items={sequences} getKey={(s) => s.name}>
          {#snippet children(seq)}
            <SequenceNode sequence={seq} schema={schemaName} {connectionId} {databaseName} {searchResults} {schemaPrefix} onRefresh={loadSequences} />
          {/snippet}
        </VirtualTreeList>
      {/snippet}
    </CategoryFolder>
  {/if}

  {#if capabilities?.indexes}
    <CategoryFolder label="Indexes" count={indexes.length} icon={ListTree} iconClass="text-teal-400" load={loadIndexes} menuItems={indexesFolderMenuItems()} menuCtx={folderMenuCtx} onmenuaction={() => handleFolderCreate('index')}>
      {#snippet children()}
        <VirtualTreeList items={indexes} getKey={(i) => i.name}>
          {#snippet children(idx)}
            <IndexNode index={idx} schema={schemaName} {connectionId} {databaseName} {searchResults} {schemaPrefix} onRefresh={loadIndexes} />
          {/snippet}
        </VirtualTreeList>
      {/snippet}
    </CategoryFolder>
  {/if}

  {#if capabilities?.foreign_tables}
    <CategoryFolder label="Foreign Tables" count={foreignTables.length} icon={ExternalLink} iconClass="text-rose-400" load={loadForeignTables}>
      {#snippet children()}
        <VirtualTreeList items={foreignTables} getKey={(ft) => ft.name}>
          {#snippet children(ft)}
            <ForeignTableNode foreignTable={ft} schema={schemaName} {connectionId} {databaseName} {searchResults} {schemaPrefix} onRefresh={loadForeignTables} />
          {/snippet}
        </VirtualTreeList>
      {/snippet}
    </CategoryFolder>
  {/if}
</div>

<CreateTableDialog bind:open={showCreateTable} schema={schemaName} {connectionId} {databaseName} oncreated={loadTables} />
<CreateViewDialog bind:open={showCreateView} schema={schemaName} {connectionId} {databaseName} oncreated={loadViews} />
<CreateViewDialog bind:open={showCreateMatView} schema={schemaName} {connectionId} {databaseName} materialized oncreated={loadMaterializedViews} />
<CreateFunctionDialog bind:open={showCreateFunction} schema={schemaName} {connectionId} {databaseName} oncreated={loadFunctions} />
<CreateSequenceDialog bind:open={showCreateSequence} schema={schemaName} {connectionId} {databaseName} oncreated={loadSequences} />
<CreateIndexDialog bind:open={showCreateIndex} schema={schemaName} {connectionId} {databaseName} oncreated={loadIndexes} />
