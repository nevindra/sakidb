<script lang="ts">
  import type { AnyQueryResult, CompareConfig, CompareMatchMode } from '$lib/types';
  import type { DiffMap } from '$lib/utils/result-diff';
  import { X } from '@lucide/svelte';
  import * as Select from '$lib/components/ui/select';

  let {
    results,
    statements,
    config,
    diffMap,
    onupdate,
    onclose,
  }: {
    results: AnyQueryResult[];
    statements: string[];
    config: CompareConfig;
    diffMap: DiffMap | null;
    onupdate: (config: CompareConfig) => void;
    onclose: () => void;
  } = $props();

  function getLabel(index: number): string {
    const sql = statements[index] ?? '';
    const trimmed = sql.trim();
    if (trimmed.length <= 30) return trimmed || `Result ${index + 1}`;
    return trimmed.slice(0, 30) + '\u2026';
  }

  // Common columns between the two selected results (for key column picker)
  let commonColumns = $derived.by(() => {
    const a = results[config.resultIndexA];
    const b = results[config.resultIndexB];
    if (!a || !b) return [];
    const bNames = new Set(b.columns.map((c) => c.name));
    return a.columns.filter((c) => bNames.has(c.name)).map((c) => c.name);
  });

  function handleResultA(value: string | undefined) {
    if (value === undefined) return;
    onupdate({ ...config, resultIndexA: Number(value) });
  }

  function handleResultB(value: string | undefined) {
    if (value === undefined) return;
    onupdate({ ...config, resultIndexB: Number(value) });
  }

  function handleMatchMode(value: string | undefined) {
    if (value === undefined) return;
    const mode = value as CompareMatchMode;
    onupdate({
      ...config,
      matchMode: mode,
      keyColumn: mode === 'key' ? commonColumns[0] : undefined,
    });
  }

  function handleKeyColumn(value: string | undefined) {
    if (value === undefined) return;
    onupdate({ ...config, keyColumn: value });
  }
</script>

<div class="flex items-center gap-3 px-2 py-1 bg-card border-b border-border text-xs shrink-0">
  <!-- Result pickers -->
  <div class="flex items-center gap-1.5">
    <span class="text-muted-foreground">Left:</span>
    <Select.Root type="single" value={String(config.resultIndexA)} onValueChange={handleResultA}>
      <Select.Trigger class="h-6 text-xs px-2 min-w-[100px] max-w-[180px]">
        {getLabel(config.resultIndexA)}
      </Select.Trigger>
      <Select.Content>
        {#each results as _, i (i)}
          <Select.Item value={String(i)} label={getLabel(i)} />
        {/each}
      </Select.Content>
    </Select.Root>
  </div>

  <span class="text-muted-foreground/40 select-none">vs</span>

  <div class="flex items-center gap-1.5">
    <span class="text-muted-foreground">Right:</span>
    <Select.Root type="single" value={String(config.resultIndexB)} onValueChange={handleResultB}>
      <Select.Trigger class="h-6 text-xs px-2 min-w-[100px] max-w-[180px]">
        {getLabel(config.resultIndexB)}
      </Select.Trigger>
      <Select.Content>
        {#each results as _, i (i)}
          <Select.Item value={String(i)} label={getLabel(i)} />
        {/each}
      </Select.Content>
    </Select.Root>
  </div>

  <span class="text-muted-foreground/40 select-none">|</span>

  <!-- Match mode -->
  <div class="flex items-center gap-1.5">
    <span class="text-muted-foreground">Match:</span>
    <Select.Root type="single" value={config.matchMode} onValueChange={handleMatchMode}>
      <Select.Trigger class="h-6 text-xs px-2 min-w-[90px]">
        {config.matchMode === 'position' ? 'By position' : 'By column'}
      </Select.Trigger>
      <Select.Content>
        <Select.Item value="position" label="By position" />
        <Select.Item value="key" label="By column" />
      </Select.Content>
    </Select.Root>
  </div>

  <!-- Key column picker (only when match mode is 'key') -->
  {#if config.matchMode === 'key' && commonColumns.length > 0}
    <div class="flex items-center gap-1.5">
      <span class="text-muted-foreground">Key:</span>
      <Select.Root type="single" value={config.keyColumn ?? commonColumns[0]} onValueChange={handleKeyColumn}>
        <Select.Trigger class="h-6 text-xs px-2 min-w-[80px] max-w-[150px]">
          {config.keyColumn ?? commonColumns[0]}
        </Select.Trigger>
        <Select.Content>
          {#each commonColumns as col (col)}
            <Select.Item value={col} label={col} />
          {/each}
        </Select.Content>
      </Select.Root>
    </div>
  {/if}

  <!-- Spacer -->
  <div class="flex-1"></div>

  <!-- Stats summary -->
  {#if diffMap}
    <span class="text-muted-foreground">
      {results[config.resultIndexA]?.row_count.toLocaleString()} vs {results[config.resultIndexB]?.row_count.toLocaleString()} rows
      {#if diffMap.changedCellCount > 0}
        &middot; <span class="text-amber-400">{diffMap.changedCellCount} cells differ</span>
      {/if}
      {#if diffMap.removedRows.size > 0}
        &middot; <span class="text-red-400">{diffMap.removedRows.size} removed</span>
      {/if}
      {#if diffMap.addedRows.size > 0}
        &middot; <span class="text-emerald-400">{diffMap.addedRows.size} added</span>
      {/if}
      {#if diffMap.changedCellCount === 0 && diffMap.removedRows.size === 0 && diffMap.addedRows.size === 0}
        &middot; <span class="text-emerald-400">Identical</span>
      {/if}
    </span>
  {/if}

  <!-- Close button -->
  <button
    class="p-0.5 rounded text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors"
    onclick={onclose}
  >
    <X class="h-3.5 w-3.5" />
  </button>
</div>
