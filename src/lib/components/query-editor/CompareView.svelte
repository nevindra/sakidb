<script lang="ts">
  import type { AnyQueryResult, CompareConfig } from '$lib/types';
  import type { DiffMap } from '$lib/utils/result-diff';
  import { diffByPosition, diffByKey } from '$lib/utils/result-diff';
  import { ColumnarResultData } from '$lib/types/query-result-data';
  import CompareToolbar from './CompareToolbar.svelte';

  let {
    results,
    statements,
    config,
    onupdate,
    onclose,
  }: {
    results: AnyQueryResult[];
    statements: string[];
    config: CompareConfig;
    onupdate: (config: CompareConfig) => void;
    onclose: () => void;
  } = $props();

  const ROW_HEIGHT = 28;
  const OVERSCAN = 8;
  const COL_WIDTH = 150;
  const ROW_NUM_WIDTH = 48;

  let resultA = $derived(results[config.resultIndexA]);
  let resultB = $derived(results[config.resultIndexB]);

  // Compute diff
  let diffMap = $derived.by((): DiffMap | null => {
    if (!resultA || !resultB) return null;
    if (config.matchMode === 'key' && config.keyColumn) {
      return diffByKey(resultA, resultB, config.keyColumn);
    }
    return diffByPosition(resultA, resultB);
  });

  // Synchronized scroll state
  let scrollTopA = $state(0);
  let scrollLeftA = $state(0);
  let scrollTopB = $state(0);
  let scrollLeftB = $state(0);
  let syncScroll = $state(true);
  let containerHeightA = $state(400);
  let containerHeightB = $state(400);
  let scrollContainerA = $state<HTMLDivElement | null>(null);
  let scrollContainerB = $state<HTMLDivElement | null>(null);
  let scrollingSource = $state<'a' | 'b' | null>(null);

  function handleScrollA(e: Event) {
    const el = e.target as HTMLDivElement;
    scrollTopA = el.scrollTop;
    scrollLeftA = el.scrollLeft;
    if (syncScroll && scrollingSource !== 'b' && scrollContainerB) {
      scrollingSource = 'a';
      scrollContainerB.scrollTop = el.scrollTop;
      scrollContainerB.scrollLeft = el.scrollLeft;
      requestAnimationFrame(() => { scrollingSource = null; });
    }
  }

  function handleScrollB(e: Event) {
    const el = e.target as HTMLDivElement;
    scrollTopB = el.scrollTop;
    scrollLeftB = el.scrollLeft;
    if (syncScroll && scrollingSource !== 'a' && scrollContainerA) {
      scrollingSource = 'b';
      scrollContainerA.scrollTop = el.scrollTop;
      scrollContainerA.scrollLeft = el.scrollLeft;
      requestAnimationFrame(() => { scrollingSource = null; });
    }
  }

  function getCellText(result: AnyQueryResult, row: number, col: number): { text: string; cls: string } {
    if (result instanceof ColumnarResultData) {
      return result.getCellDisplay(row, col);
    }
    const qr = result;
    const cell = qr.cells[row * qr.columns.length + col];
    if (cell === 'Null') return { text: 'NULL', cls: 'text-text-dim italic' };
    if ('Bool' in cell) return { text: String(cell.Bool), cls: 'text-warning' };
    if ('Int' in cell) return { text: String(cell.Int), cls: 'text-right tabular-nums' };
    if ('Float' in cell) return { text: String(cell.Float), cls: 'text-right tabular-nums' };
    if ('Text' in cell) return { text: cell.Text.length > 200 ? cell.Text.slice(0, 200) + '\u2026' : cell.Text, cls: '' };
    if ('Json' in cell) return { text: cell.Json.length > 200 ? cell.Json.slice(0, 200) + '\u2026' : cell.Json, cls: 'font-mono text-primary' };
    if ('Timestamp' in cell) return { text: cell.Timestamp, cls: 'text-success' };
    if ('Bytes' in cell) {
      const hex = cell.Bytes.slice(0, 16).map(b => b.toString(16).padStart(2, '0')).join(' ');
      return { text: `\\x${hex}`, cls: 'font-mono text-text-dim' };
    }
    return { text: '', cls: '' };
  }

  function getVisibleRows(scrollTop: number, containerHeight: number, totalRows: number) {
    const startRow = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN);
    const endRow = Math.min(totalRows, Math.ceil((scrollTop + containerHeight) / ROW_HEIGHT) + OVERSCAN);
    return { startRow, endRow };
  }
</script>

<div class="flex flex-col h-full overflow-hidden">
  <CompareToolbar
    {results}
    {statements}
    {config}
    {diffMap}
    {onupdate}
    {onclose}
  />

  <div class="flex flex-1 overflow-hidden">
    <!-- Left panel (Result A) -->
    {#if resultA}
      {@const visA = getVisibleRows(scrollTopA, containerHeightA, resultA.row_count)}
      <div class="flex-1 flex flex-col overflow-hidden border-r border-border min-w-0">
        <div class="px-2 py-0.5 text-[11px] text-muted-foreground bg-card/50 border-b border-border shrink-0 truncate">
          {statements[config.resultIndexA] ?? `Result ${config.resultIndexA + 1}`}
        </div>
        <div
          class="flex-1 overflow-auto"
          bind:this={scrollContainerA}
          onscroll={handleScrollA}
          bind:clientHeight={containerHeightA}
        >
          <div style="width: {ROW_NUM_WIDTH + resultA.columns.length * COL_WIDTH}px; height: {resultA.row_count * ROW_HEIGHT + ROW_HEIGHT}px; position: relative;">
            <!-- Header -->
            <div class="sticky top-0 z-10 flex bg-muted/80 backdrop-blur-sm border-b border-border" style="height: {ROW_HEIGHT}px;">
              <div class="shrink-0 flex items-center justify-center text-[10px] text-muted-foreground border-r border-border" style="width: {ROW_NUM_WIDTH}px;">#</div>
              {#each resultA.columns as col, ci (ci)}
                <div class="shrink-0 flex items-center px-2 text-xs font-medium text-foreground border-r border-border truncate" style="width: {COL_WIDTH}px;" title="{col.name} ({col.data_type})">
                  {col.name}
                </div>
              {/each}
            </div>
            <!-- Rows -->
            {#each { length: visA.endRow - visA.startRow } as _, i (visA.startRow + i)}
              {@const row = visA.startRow + i}
              {@const isRemoved = diffMap?.removedRows.has(row)}
              <div
                class="absolute flex border-b border-border/50 {isRemoved ? 'bg-red-500/10' : ''}"
                style="top: {(row + 1) * ROW_HEIGHT}px; height: {ROW_HEIGHT}px; width: {ROW_NUM_WIDTH + resultA.columns.length * COL_WIDTH}px;"
              >
                <div class="shrink-0 flex items-center justify-center text-[10px] text-muted-foreground border-r border-border" style="width: {ROW_NUM_WIDTH}px;">
                  {row + 1}
                </div>
                {#each resultA.columns as _, ci (ci)}
                  {@const display = getCellText(resultA, row, ci)}
                  {@const isChanged = diffMap?.changed.has(`${row}:${ci}`)}
                  <div
                    class="shrink-0 flex items-center px-2 text-xs border-r border-border/30 truncate {display.cls} {isChanged ? 'bg-amber-500/15' : ''}"
                    style="width: {COL_WIDTH}px;"
                  >
                    {display.text}
                  </div>
                {/each}
              </div>
            {/each}
          </div>
        </div>
      </div>
    {/if}

    <!-- Right panel (Result B) -->
    {#if resultB}
      {@const visB = getVisibleRows(scrollTopB, containerHeightB, resultB.row_count)}
      <div class="flex-1 flex flex-col overflow-hidden min-w-0">
        <div class="px-2 py-0.5 text-[11px] text-muted-foreground bg-card/50 border-b border-border shrink-0 truncate">
          {statements[config.resultIndexB] ?? `Result ${config.resultIndexB + 1}`}
        </div>
        <div
          class="flex-1 overflow-auto"
          bind:this={scrollContainerB}
          onscroll={handleScrollB}
          bind:clientHeight={containerHeightB}
        >
          <div style="width: {ROW_NUM_WIDTH + resultB.columns.length * COL_WIDTH}px; height: {resultB.row_count * ROW_HEIGHT + ROW_HEIGHT}px; position: relative;">
            <!-- Header -->
            <div class="sticky top-0 z-10 flex bg-muted/80 backdrop-blur-sm border-b border-border" style="height: {ROW_HEIGHT}px;">
              <div class="shrink-0 flex items-center justify-center text-[10px] text-muted-foreground border-r border-border" style="width: {ROW_NUM_WIDTH}px;">#</div>
              {#each resultB.columns as col, ci (ci)}
                <div class="shrink-0 flex items-center px-2 text-xs font-medium text-foreground border-r border-border truncate" style="width: {COL_WIDTH}px;" title="{col.name} ({col.data_type})">
                  {col.name}
                </div>
              {/each}
            </div>
            <!-- Rows -->
            {#each { length: visB.endRow - visB.startRow } as _, i (visB.startRow + i)}
              {@const row = visB.startRow + i}
              {@const isAdded = diffMap?.addedRows.has(row)}
              <div
                class="absolute flex border-b border-border/50 {isAdded ? 'bg-emerald-500/10' : ''}"
                style="top: {(row + 1) * ROW_HEIGHT}px; height: {ROW_HEIGHT}px; width: {ROW_NUM_WIDTH + resultB.columns.length * COL_WIDTH}px;"
              >
                <div class="shrink-0 flex items-center justify-center text-[10px] text-muted-foreground border-r border-border" style="width: {ROW_NUM_WIDTH}px;">
                  {row + 1}
                </div>
                {#each resultB.columns as _, ci (ci)}
                  {@const display = getCellText(resultB, row, ci)}
                  {@const isChanged = diffMap?.changed.has(`${row}:${ci}`)}
                  <div
                    class="shrink-0 flex items-center px-2 text-xs border-r border-border/30 truncate {display.cls} {isChanged ? 'bg-amber-500/15' : ''}"
                    style="width: {COL_WIDTH}px;"
                  >
                    {display.text}
                  </div>
                {/each}
              </div>
            {/each}
          </div>
        </div>
      </div>
    {/if}
  </div>
</div>
