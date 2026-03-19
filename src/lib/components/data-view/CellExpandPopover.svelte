<script lang="ts">
  import type { CellValue, ColumnDef } from '$lib/types';
  import { cellToPlainText, copyToClipboard } from '$lib/copy-utils';
  import { detectBinaryFormat } from '$lib/binary-utils';
  import { getTypeCategory } from '$lib/type-utils';
  import BinaryPreview from './BinaryPreview.svelte';

  let {
    cell,
    column,
    anchorRect,
    containerRect,
    onclose,
  }: {
    cell: CellValue;
    column: ColumnDef;
    anchorRect: DOMRect;
    containerRect: DOMRect;
    onclose: () => void;
  } = $props();

  let popoverEl = $state<HTMLDivElement | null>(null);
  let copied = $state(false);

  const formattedValue = $derived.by(() => {
    if (cell === 'Null') return null;
    if ('Json' in cell) {
      try {
        return JSON.stringify(JSON.parse(cell.Json), null, 2);
      } catch {
        return cell.Json;
      }
    }
    if ('Bytes' in cell) {
      const lines: string[] = [];
      for (let i = 0; i < cell.Bytes.length; i += 16) {
        const offset = i.toString(16).padStart(8, '0');
        const hex = cell.Bytes.slice(i, i + 16).map(b => b.toString(16).padStart(2, '0')).join(' ');
        lines.push(`${offset}  ${hex}`);
      }
      return lines.join('\n');
    }
    return cellToPlainText(cell);
  });

  const isJson = $derived(cell !== 'Null' && 'Json' in cell);
  const isText = $derived(cell !== 'Null' && typeof cell === 'object' && 'Text' in cell);
  const isNull = $derived(cell === 'Null');
  const isBytes = $derived(cell !== 'Null' && typeof cell === 'object' && 'Bytes' in cell);
  const bytesFormat = $derived(isBytes ? detectBinaryFormat((cell as { Bytes: number[] }).Bytes) : null);
  const hasBinaryPreview = $derived(bytesFormat !== null && bytesFormat.kind !== 'unknown');
  const category = $derived(getTypeCategory(column.data_type));
  const isXml = $derived(category === 'xml');

  // Larger popover for PDF
  const isPdf = $derived(bytesFormat?.kind === 'pdf');
  const baseMaxW = $derived(isPdf ? 560 : 400);
  const baseMaxH = $derived(isPdf ? 480 : 300);

  // Position: prefer below-right of anchor, stay close to cell
  const MARGIN = 4;
  const effectiveMaxW = $derived(Math.min(baseMaxW, containerRect.width - MARGIN * 2));
  const effectiveMaxH = $derived(Math.min(baseMaxH, containerRect.height - MARGIN * 2));

  const position = $derived.by(() => {
    const cellTop = anchorRect.top - containerRect.top;
    const cellBottom = anchorRect.bottom - containerRect.top;
    const cellLeft = anchorRect.left - containerRect.left;

    const spaceBelow = containerRect.height - cellBottom;
    const spaceAbove = cellTop;

    // Vertical: use top when below, bottom when above (so popover hugs the cell)
    let vertical: { top: string; bottom: string };
    if (spaceBelow >= effectiveMaxH + MARGIN || spaceBelow >= spaceAbove) {
      // Below: anchor popover top to cell bottom
      const top = Math.max(MARGIN, cellBottom + MARGIN);
      vertical = { top: `${top}px`, bottom: 'auto' };
    } else {
      // Above: anchor popover bottom to cell top (content grows upward)
      const bottom = Math.max(MARGIN, containerRect.height - cellTop + MARGIN);
      vertical = { top: 'auto', bottom: `${bottom}px` };
    }

    // Horizontal: align with cell left, or anchor to cell right if overflows
    const cellRight = anchorRect.right - containerRect.left;
    let left = cellLeft;
    if (left + effectiveMaxW > containerRect.width - MARGIN) {
      // Align popover right edge with cell right edge
      left = cellRight - effectiveMaxW;
    }
    left = Math.max(MARGIN, left);

    return { ...vertical, left };
  });

  async function handleCopy() {
    const text = isJson && formattedValue ? formattedValue : cellToPlainText(cell);
    const ok = await copyToClipboard(text);
    if (ok) {
      copied = true;
      setTimeout(() => (copied = false), 1500);
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      onclose();
    }
  }

  function handleClickOutside(e: MouseEvent) {
    if (popoverEl && !popoverEl.contains(e.target as Node)) {
      onclose();
    }
  }

  function setupClickOutside(node: HTMLDivElement) {
    // Delay to avoid the same click that opened this from closing it
    const timer = setTimeout(() => {
      document.addEventListener('mousedown', handleClickOutside);
    }, 0);
    return {
      destroy() {
        clearTimeout(timer);
        document.removeEventListener('mousedown', handleClickOutside);
      },
    };
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div
  bind:this={popoverEl}
  use:setupClickOutside
  class="absolute z-30 border border-border bg-card rounded-md shadow-xl shadow-black/40 overflow-hidden"
  style="top: {position.top}; bottom: {position.bottom}; left: {position.left}px; max-width: {effectiveMaxW}px; max-height: {effectiveMaxH}px;"
>
  <!-- Header -->
  <div class="flex items-center justify-between px-3 py-1.5 border-b border-border bg-card">
    <div class="flex items-center gap-2 text-xs">
      <span class="font-medium text-foreground">{column.name}</span>
      <span class="text-text-dim text-[10px]">{column.data_type}</span>
    </div>
    {#if !hasBinaryPreview}
      <button
        class="text-xs px-2 py-0.5 rounded text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors"
        onclick={handleCopy}
      >
        {copied ? 'Copied' : 'Copy'}
      </button>
    {/if}
  </div>

  <!-- Content -->
  <div class="overflow-auto p-3 text-xs" style="max-height: {effectiveMaxH - 40}px;">
    {#if isBytes && hasBinaryPreview}
      <BinaryPreview
        bytes={(cell as { Bytes: number[] }).Bytes}
        columnName={column.name}
        maxImageHeight={isPdf ? 0 : effectiveMaxH - 80}
        pdfHeight={effectiveMaxH - 80}
      />
    {:else if isNull}
      <span class="text-text-dim italic">NULL</span>
    {:else if isJson}
      <pre class="font-mono text-primary whitespace-pre-wrap break-words">{formattedValue}</pre>
    {:else if isText}
      <pre class="font-mono whitespace-pre-wrap break-words">{formattedValue}</pre>
    {:else if isXml}
      <pre class="font-mono text-primary whitespace-pre-wrap break-words">{formattedValue}</pre>
    {:else if isBytes}
      <pre class="font-mono text-text-dim whitespace-pre">{formattedValue}</pre>
    {:else if category === 'identifier' || category === 'network'}
      <pre class="font-mono whitespace-pre-wrap break-all {category === 'identifier' ? 'text-sky-400/80' : 'text-teal-400/80'}">{formattedValue}</pre>
    {:else if category === 'geometric'}
      <pre class="font-mono text-violet-400/80 whitespace-pre-wrap break-words">{formattedValue}</pre>
    {:else if category === 'search'}
      <pre class="text-amber-400/80 whitespace-pre-wrap break-words">{formattedValue}</pre>
    {:else}
      <div class="whitespace-pre-wrap break-words">{formattedValue}</div>
    {/if}
  </div>
</div>
