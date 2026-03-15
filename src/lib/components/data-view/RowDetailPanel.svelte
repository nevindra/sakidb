<script lang="ts">
  import type { CellValue, ColumnDef } from '$lib/types';
  import { cellToPlainText, rowToJson, copyToClipboard } from '$lib/copy-utils';
  import { detectBinaryFormat } from '$lib/binary-utils';
  import BinaryPreview from './BinaryPreview.svelte';
  import * as Sheet from '$lib/components/ui/sheet';
  import { Pencil } from '@lucide/svelte';

  let {
    open = $bindable(false),
    row,
    rowIndex,
    columns,
    onnavigate,
    totalRows,
    canEdit = false,
    onedit,
  }: {
    open: boolean;
    row: CellValue[] | null;
    rowIndex: number;
    columns: ColumnDef[];
    onnavigate: (direction: 'prev' | 'next') => void;
    totalRows: number;
    canEdit?: boolean;
    onedit?: () => void;
  } = $props();

  let copiedField = $state<number | null>(null);
  let copiedRow = $state(false);
  let viewMode = $state<'fields' | 'json'>('fields');

  async function copyField(idx: number) {
    if (!row) return;
    const cell = row[idx];
    let text: string;
    if (cell !== 'Null' && 'Json' in cell) {
      try {
        text = JSON.stringify(JSON.parse(cell.Json), null, 2);
      } catch {
        text = cell.Json;
      }
    } else {
      text = cellToPlainText(cell);
    }
    const ok = await copyToClipboard(text);
    if (ok) {
      copiedField = idx;
      setTimeout(() => (copiedField = null), 1500);
    }
  }

  async function copyRowAsJson() {
    if (!row) return;
    const ok = await copyToClipboard(rowToJson(row, columns));
    if (ok) {
      copiedRow = true;
      setTimeout(() => (copiedRow = false), 1500);
    }
  }

  function formatCellDisplay(cell: CellValue): { text: string; cls: string } {
    if (cell === 'Null') return { text: 'NULL', cls: 'text-text-dim italic' };
    if ('Bool' in cell) return { text: String(cell.Bool), cls: 'text-warning' };
    if ('Int' in cell) return { text: String(cell.Int), cls: 'tabular-nums' };
    if ('Float' in cell) return { text: String(cell.Float), cls: 'tabular-nums' };
    if ('Text' in cell) return { text: cell.Text, cls: '' };
    if ('Json' in cell) {
      try {
        return { text: JSON.stringify(JSON.parse(cell.Json), null, 2), cls: 'font-mono text-primary whitespace-pre-wrap' };
      } catch {
        return { text: cell.Json, cls: 'font-mono text-primary' };
      }
    }
    if ('Timestamp' in cell) return { text: cell.Timestamp, cls: 'text-success' };
    if ('Bytes' in cell) {
      const hex = cell.Bytes.map(b => b.toString(16).padStart(2, '0')).join(' ');
      return { text: hex, cls: 'font-mono text-text-dim' };
    }
    return { text: '?', cls: '' };
  }

  function isBinaryPreviewable(cell: CellValue): boolean {
    if (cell === 'Null' || !('Bytes' in cell)) return false;
    return detectBinaryFormat(cell.Bytes).kind !== 'unknown';
  }

  function handleKeydown(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === 'ArrowUp' || e.key === 'k') {
      e.preventDefault();
      onnavigate('prev');
    } else if (e.key === 'ArrowDown' || e.key === 'j') {
      e.preventDefault();
      onnavigate('next');
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<Sheet.Root bind:open>
  <Sheet.Content side="right" class="w-[420px] sm:max-w-[420px] flex flex-col">
    <Sheet.Header class="shrink-0 px-4 pt-4 pb-2">
      <div class="flex items-center justify-between">
        <Sheet.Title class="text-sm font-medium">Row {rowIndex + 1}</Sheet.Title>
        <div class="flex items-center gap-1 mr-6">
          {#if canEdit && onedit}
            <button
              class="px-1.5 py-0.5 rounded text-xs text-primary hover:bg-primary/10 transition-colors flex items-center gap-1"
              onclick={onedit}
              title="Edit this row"
            >
              <Pencil class="h-3 w-3" />
            </button>
            <div class="w-px h-3 bg-border/40"></div>
          {/if}
          <button
            class="px-1.5 py-0.5 rounded text-xs text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors disabled:opacity-30"
            onclick={() => onnavigate('prev')}
            disabled={rowIndex <= 0}
          >
            ↑
          </button>
          <span class="text-xs text-text-dim tabular-nums">{rowIndex + 1}/{totalRows}</span>
          <button
            class="px-1.5 py-0.5 rounded text-xs text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors disabled:opacity-30"
            onclick={() => onnavigate('next')}
            disabled={rowIndex >= totalRows - 1}
          >
            ↓
          </button>
        </div>
      </div>
      <Sheet.Description class="sr-only">Row detail view showing all column values</Sheet.Description>
      <div class="flex items-center gap-0.5 mt-1 p-0.5 rounded-md bg-accent/20 w-fit">
        <button
          class="px-2.5 py-0.5 rounded text-[11px] font-medium transition-colors {viewMode === 'fields' ? 'bg-accent text-foreground' : 'text-muted-foreground hover:text-foreground'}"
          onclick={() => (viewMode = 'fields')}
        >
          Fields
        </button>
        <button
          class="px-2.5 py-0.5 rounded text-[11px] font-medium transition-colors {viewMode === 'json' ? 'bg-accent text-foreground' : 'text-muted-foreground hover:text-foreground'}"
          onclick={() => (viewMode = 'json')}
        >
          JSON
        </button>
      </div>
    </Sheet.Header>

    <div class="flex-1 overflow-y-auto px-4 pb-4">
      {#if row}
        {#if viewMode === 'fields'}
          <div class="space-y-0">
            {#each columns as col, i}
              {@const cell = row[i]}
              {@const display = formatCellDisplay(cell)}
              {@const hasBinary = isBinaryPreviewable(cell)}
              <div class="group py-2 border-b border-border/50 last:border-b-0">
                <div class="flex items-center justify-between mb-0.5">
                  <div class="flex items-center gap-2">
                    <span class="text-xs text-muted-foreground font-medium">{col.name}</span>
                    <span class="text-[10px] text-text-dim">{col.data_type}</span>
                  </div>
                  {#if !hasBinary}
                    <button
                      class="text-[10px] px-1.5 py-0.5 rounded text-muted-foreground opacity-0 group-hover:opacity-100 hover:text-foreground hover:bg-accent/50 transition-all"
                      onclick={() => copyField(i)}
                    >
                      {copiedField === i ? 'Copied' : 'Copy'}
                    </button>
                  {/if}
                </div>
                {#if hasBinary}
                  <BinaryPreview
                    bytes={(cell as { Bytes: number[] }).Bytes}
                    columnName={col.name}
                    maxImageHeight={240}
                    pdfHeight={400}
                  />
                {:else}
                  <div class="text-xs break-words {display.cls}">{display.text}</div>
                {/if}
              </div>
            {/each}
          </div>
        {:else}
          <pre class="text-xs font-mono text-primary whitespace-pre-wrap break-words pt-2">{rowToJson(row, columns)}</pre>
        {/if}
      {/if}
    </div>

    <Sheet.Footer class="shrink-0 border-t border-border px-4 py-2">
      <button
        class="text-xs px-3 py-1.5 rounded bg-accent/30 text-foreground hover:bg-accent/50 transition-colors"
        onclick={copyRowAsJson}
      >
        {copiedRow ? 'Copied' : 'Copy Row as JSON'}
      </button>
    </Sheet.Footer>
  </Sheet.Content>
</Sheet.Root>
