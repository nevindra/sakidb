<script lang="ts">
  import type { CellValue, AnyQueryResult } from '$lib/types';
  import { ColumnarResultData } from '$lib/types/query-result-data';
  import { parseExplain } from '$lib/utils/explain-parser';
  import ExplainTableView from './ExplainTableView.svelte';
  import ExplainTreeView from './ExplainTreeView.svelte';
  import DataGrid from '../data-view/DataGrid.svelte';
  import { Table2, GitBranchPlus, FileText } from '@lucide/svelte';

  let { result, onshowraw }: { result: AnyQueryResult; onshowraw?: () => void } = $props();

  type ViewMode = 'table' | 'tree' | 'raw';
  let viewMode: ViewMode = $state('table');

  function extractCellText(cell: CellValue): string {
    if (cell === 'Null') return '';
    if (typeof cell === 'object' && cell !== null) {
      if ('Text' in cell) return cell.Text;
      if ('Json' in cell) return cell.Json;
      if ('Bool' in cell) return String(cell.Bool);
      if ('Int' in cell) return String(cell.Int);
      if ('Float' in cell) return String(cell.Float);
      if ('Timestamp' in cell) return cell.Timestamp;
      if ('Bytes' in cell) return '[bytes]';
    }
    return '';
  }

  let explainText = $derived.by(() => {
    if (result instanceof ColumnarResultData) {
      // EXPLAIN results are single-column text — read directly from columnData
      const cd = result.columnData[0];
      if (cd.type !== 'text') return '';
      const lines: string[] = [];
      for (let i = 0; i < result.row_count; i++) {
        lines.push(cd.nulls[i] !== 0 ? '' : (result.getValue(i, 0) as string));
      }
      return lines.join('\n');
    }
    return result.cells.map((cell) => extractCellText(cell)).join('\n');
  });

  let plan = $derived(parseExplain(explainText));

</script>

<div class="flex flex-col h-full overflow-hidden">
  <!-- View toggle bar -->
  <div class="flex items-center h-7 px-2 gap-1 bg-card border-b border-border shrink-0">
    <button
      class="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs transition-colors
        {viewMode === 'table'
          ? 'bg-accent text-foreground'
          : 'text-muted-foreground hover:text-foreground hover:bg-accent/30'}"
      onclick={() => (viewMode = 'table')}
    >
      <Table2 class="h-3 w-3" />
      Table
    </button>
    <button
      class="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs transition-colors
        {viewMode === 'tree'
          ? 'bg-accent text-foreground'
          : 'text-muted-foreground hover:text-foreground hover:bg-accent/30'}"
      onclick={() => (viewMode = 'tree')}
    >
      <GitBranchPlus class="h-3 w-3" />
      Tree
    </button>
    <button
      class="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs transition-colors
        {viewMode === 'raw'
          ? 'bg-accent text-foreground'
          : 'text-muted-foreground hover:text-foreground hover:bg-accent/30'}"
      onclick={() => (viewMode = 'raw')}
    >
      <FileText class="h-3 w-3" />
      Raw
    </button>
  </div>

  <!-- Content -->
  {#if viewMode === 'raw'}
    <div class="flex-1 overflow-hidden">
      <DataGrid {result} />
    </div>
  {:else if plan}
    <div class="flex-1 overflow-hidden">
      {#if viewMode === 'table'}
        <ExplainTableView {plan} />
      {:else}
        <ExplainTreeView {plan} />
      {/if}
    </div>
  {:else}
    <div class="flex-1 flex items-center justify-center text-xs text-muted-foreground">
      Unable to parse EXPLAIN output
    </div>
  {/if}
</div>
