<script lang="ts">
  import type { CellValue, ColumnDef } from '$lib/types';
  import { cellToPlainText, cellToJsonValue, rowToJson, rowToCsv, copyToClipboard } from '$lib/copy-utils';

  let {
    x,
    y,
    cell,
    row,
    columns,
    columnName,
    canEdit,
    onclose,
    oneditcell,
    oninsertrow,
    ondeleterow,
    onviewdetails,
  }: {
    x: number;
    y: number;
    cell: CellValue;
    row: CellValue[];
    columns: ColumnDef[];
    columnName: string;
    canEdit: boolean;
    onclose: () => void;
    oneditcell: () => void;
    oninsertrow: () => void;
    ondeleterow: () => void;
    onviewdetails: () => void;
  } = $props();

  async function copyCellValue() {
    await copyToClipboard(cellToPlainText(cell));
    onclose();
  }

  async function copyCellAsJson() {
    await copyToClipboard(JSON.stringify(cellToJsonValue(cell), null, 2));
    onclose();
  }

  async function copyRowAsJson() {
    await copyToClipboard(rowToJson(row, columns));
    onclose();
  }

  async function copyRowAsCsv() {
    await copyToClipboard(rowToCsv(row, columns));
    onclose();
  }

  async function copyColumnName() {
    await copyToClipboard(columnName);
    onclose();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_interactive_supports_focus, a11y_click_events_have_key_events -->
<div
  role="menu"
  class="fixed z-50 min-w-[12rem] overflow-hidden rounded-lg border border-border/60 bg-popover p-1 text-popover-foreground shadow-xl shadow-black/30"
  style="left: {x}px; top: {y}px;"
  onclick={(e) => e.stopPropagation()}
>
  <button class="relative flex w-full cursor-default select-none items-center rounded-sm px-2 py-1.5 text-xs outline-none hover:bg-accent hover:text-accent-foreground" onclick={copyCellValue}>
    Copy Cell Value
  </button>
  <button class="relative flex w-full cursor-default select-none items-center rounded-sm px-2 py-1.5 text-xs outline-none hover:bg-accent hover:text-accent-foreground" onclick={copyCellAsJson}>
    Copy Cell as JSON
  </button>
  <div class="my-1 h-px bg-border/60"></div>
  <button class="relative flex w-full cursor-default select-none items-center rounded-sm px-2 py-1.5 text-xs outline-none hover:bg-accent hover:text-accent-foreground" onclick={copyRowAsJson}>
    Copy Row as JSON
  </button>
  <button class="relative flex w-full cursor-default select-none items-center rounded-sm px-2 py-1.5 text-xs outline-none hover:bg-accent hover:text-accent-foreground" onclick={copyRowAsCsv}>
    Copy Row as CSV
  </button>
  <div class="my-1 h-px bg-border/60"></div>
  <button class="relative flex w-full cursor-default select-none items-center rounded-sm px-2 py-1.5 text-xs outline-none hover:bg-accent hover:text-accent-foreground" onclick={copyColumnName}>
    Copy Column Name
  </button>
  <div class="my-1 h-px bg-border/60"></div>
  <button class="relative flex w-full cursor-default select-none items-center rounded-sm px-2 py-1.5 text-xs outline-none hover:bg-accent hover:text-accent-foreground" onclick={() => { onviewdetails(); onclose(); }}>
    View Row Details
  </button>
  {#if canEdit}
    <div class="my-1 h-px bg-border/60"></div>
    <button class="relative flex w-full cursor-default select-none items-center rounded-sm px-2 py-1.5 text-xs outline-none hover:bg-accent hover:text-accent-foreground" onclick={() => { oneditcell(); onclose(); }}>
      Edit Cell
    </button>
    <button class="relative flex w-full cursor-default select-none items-center rounded-sm px-2 py-1.5 text-xs outline-none hover:bg-accent hover:text-accent-foreground" onclick={() => { oninsertrow(); onclose(); }}>
      Insert Row
    </button>
    <button class="relative flex w-full cursor-default select-none items-center rounded-sm px-2 py-1.5 text-xs outline-none text-destructive hover:bg-destructive/10 hover:text-destructive" onclick={() => { ondeleterow(); onclose(); }}>
      Delete Row
    </button>
  {/if}
</div>
