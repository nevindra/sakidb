<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { QueryTab } from '$lib/types';
  import { Button } from '$lib/components/ui/button';
  import { Play, Square, Loader2, Database, ChevronDown, Settings2, Check, WrapText } from '@lucide/svelte';
  import * as Select from '$lib/components/ui/select';

  let {
    tab,
    elapsedMs,
    formatElapsed,
    onExecute,
    hasSelection,
    onExecuteStatement,
    onExplainAnalyze,
    onExplainAnalyzeJson,
    onFormat,
  }: {
    tab: QueryTab;
    elapsedMs: number;
    formatElapsed: (ms: number) => string;
    onExecute: () => void;
    hasSelection: boolean;
    onExecuteStatement: () => void;
    onExplainAnalyze: () => void;
    onExplainAnalyzeJson: () => void;
    onFormat: () => void;
  } = $props();

  const app = getAppState();

  let switchingDb = $state(false);
  let runMenuOpen = $state(false);
  let settingsOpen = $state(false);

  let runMenuEl: HTMLDivElement | undefined = $state();
  let settingsMenuEl: HTMLDivElement | undefined = $state();

  const timeoutOptions: { label: string; value: number | null }[] = [
    { label: 'No limit', value: null },
    { label: '15s', value: 15000 },
    { label: '30s', value: 30000 },
    { label: '1m', value: 60000 },
    { label: '2m', value: 120000 },
    { label: '5m', value: 300000 },
  ];

  function handleWindowClick(e: MouseEvent) {
    if (runMenuOpen && runMenuEl && !runMenuEl.contains(e.target as Node)) {
      runMenuOpen = false;
    }
    if (settingsOpen && settingsMenuEl && !settingsMenuEl.contains(e.target as Node)) {
      settingsOpen = false;
    }
  }

  // Get the connection info for this tab
  const capabilities = $derived(app.getCapabilities(tab.savedConnectionId));
  let connection = $derived(app.activeConnections.get(tab.savedConnectionId));
  let databases = $derived(connection?.databases ?? []);
  let schemas = $derived(
    connection?.activeDatabases.get(tab.databaseName)?.schemas ?? []
  );

  async function handleDatabaseChange(dbName: string) {
    if (dbName === tab.databaseName) return;
    switchingDb = true;
    try {
      await app.switchQueryTabDatabase(tab.id, dbName);
    } finally {
      switchingDb = false;
    }
  }

  function handleSchemaChange(schemaName: string) {
    if (schemaName === tab.schemaName) return;
    app.switchQueryTabSchema(tab.id, schemaName);
  }
</script>

<svelte:window onclick={handleWindowClick} />

<div class="flex items-center gap-1.5 px-2 h-8 bg-card/50 border-b border-border shrink-0">
  <!-- Database selector (multi-database engines only) -->
  {#if capabilities?.multi_database !== false}
    <Select.Root
      type="single"
      value={tab.databaseName}
      onValueChange={handleDatabaseChange}
    >
      <Select.Trigger
        size="sm"
        class="h-6 px-2 text-xs border-none shadow-none bg-transparent hover:bg-accent gap-1 min-w-0"
      >
        {#if switchingDb}
          <Loader2 class="h-3 w-3 animate-spin text-muted-foreground shrink-0" />
        {:else}
          <Database class="h-3 w-3 text-muted-foreground shrink-0" />
        {/if}
        <span class="truncate max-w-[140px]">{tab.databaseName}</span>
      </Select.Trigger>
      <Select.Content>
        {#each databases as db (db.name)}
          {@const isConnected = connection?.activeDatabases.has(db.name)}
          <Select.Item
            value={db.name}
            class={isConnected ? '' : 'text-muted-foreground'}
          >
            {db.name}
          </Select.Item>
        {/each}
      </Select.Content>
    </Select.Root>
  {/if}

  {#if capabilities?.multi_database !== false && capabilities?.schemas !== false}
    <span class="text-muted-foreground/40 text-xs select-none">/</span>
  {/if}

  <!-- Schema selector (engines with schemas only) -->
  {#if capabilities?.schemas !== false}
    <Select.Root
      type="single"
      value={tab.schemaName}
      onValueChange={handleSchemaChange}
    >
      <Select.Trigger
        size="sm"
        class="h-6 px-2 text-xs border-none shadow-none bg-transparent hover:bg-accent gap-1 min-w-0"
      >
        <span class="truncate max-w-[100px]">{tab.schemaName}</span>
      </Select.Trigger>
      <Select.Content>
        {#each schemas as schema (schema.name)}
          <Select.Item value={schema.name}>{schema.name}</Select.Item>
        {/each}
      </Select.Content>
    </Select.Root>
  {/if}

  <!-- Format -->
  <button
    class="p-1 h-6 w-6 rounded text-muted-foreground hover:bg-accent flex items-center justify-center"
    onclick={onFormat}
    title="Format SQL (Ctrl+Shift+F)"
  >
    <WrapText class="h-3.5 w-3.5" />
  </button>

  <!-- Spacer -->
  <div class="flex-1"></div>

  <!-- Elapsed + Run/Cancel -->
  {#if tab.isExecuting}
    <span class="text-warning text-xs font-medium tabular-nums px-1">{formatElapsed(elapsedMs)}</span>
    <Button
      size="sm"
      variant="destructive"
      class="h-6 text-xs px-2"
      onclick={() => app.cancelQuery(tab.id)}
    >
      <Square class="h-3 w-3 mr-1" />
      Cancel
    </Button>
  {:else}
    <!-- Split Run button -->
    <div class="relative flex items-center" bind:this={runMenuEl}>
      <Button
        size="sm"
        class="h-6 text-xs px-2 rounded-r-none"
        onclick={onExecute}
      >
        <Play class="h-3 w-3 mr-1" />
        {hasSelection ? 'Run Selection' : 'Run'}
        <kbd class="ml-1.5 text-[10px] opacity-60">⌘↵</kbd>
      </Button>
      <button
        class="h-6 px-1 border-l border-primary-foreground/20 bg-primary text-primary-foreground rounded-r-md hover:bg-primary/90 flex items-center"
        onclick={() => (runMenuOpen = !runMenuOpen)}
      >
        <ChevronDown class="h-3 w-3" />
      </button>

      {#if runMenuOpen}
        <div class="absolute top-full right-0 mt-1 z-50 bg-popover border border-border rounded-md shadow-lg py-1 min-w-[200px]">
          <button
            class="w-full px-3 py-1.5 text-xs hover:bg-accent/50 cursor-pointer flex items-center justify-between"
            onclick={() => { onExecute(); runMenuOpen = false; }}
          >
            <span>Run All</span>
            <span class="text-muted-foreground">⌘↵</span>
          </button>
          <button
            class="w-full px-3 py-1.5 text-xs hover:bg-accent/50 cursor-pointer flex items-center justify-between"
            onclick={() => { onExecuteStatement(); runMenuOpen = false; }}
          >
            <span>Run Current Statement</span>
            <span class="text-muted-foreground">⌘⇧↵</span>
          </button>
          {#if capabilities?.explain !== false}
            <div class="border-t border-border my-1"></div>
            <button
              class="w-full px-3 py-1.5 text-xs hover:bg-accent/50 cursor-pointer flex items-center justify-between"
              onclick={() => { onExplainAnalyze(); runMenuOpen = false; }}
            >
              <span>Explain Analyze</span>
            </button>
            <button
              class="w-full px-3 py-1.5 text-xs hover:bg-accent/50 cursor-pointer flex items-center justify-between"
              onclick={() => { onExplainAnalyzeJson(); runMenuOpen = false; }}
            >
              <span>Explain Analyze (JSON)</span>
            </button>
          {/if}
        </div>
      {/if}
    </div>
  {/if}

  <!-- Settings gear (statement timeout) -->
  <div class="relative" bind:this={settingsMenuEl}>
    <button
      class="p-1 h-6 w-6 rounded text-muted-foreground hover:bg-accent flex items-center justify-center"
      onclick={() => (settingsOpen = !settingsOpen)}
    >
      <Settings2 class="h-3.5 w-3.5" />
    </button>

    {#if settingsOpen}
      <div class="absolute top-full right-0 mt-1 z-50 bg-popover border border-border rounded-md shadow-lg py-1 min-w-[200px]">
        <div class="text-[11px] text-muted-foreground px-3 py-1">Statement Timeout</div>
        {#each timeoutOptions as opt (opt.label)}
          {@const isActive = tab.statementTimeoutMs === opt.value}
          <button
            class="w-full px-3 py-1.5 text-xs hover:bg-accent/50 cursor-pointer flex items-center justify-between {isActive ? 'text-primary' : ''}"
            onclick={() => { app.updateQueryTabTimeout(tab.id, opt.value); settingsOpen = false; }}
          >
            <span>{opt.label}</span>
            {#if isActive}
              <Check class="h-3 w-3" />
            {/if}
          </button>
        {/each}
      </div>
    {/if}
  </div>
</div>
