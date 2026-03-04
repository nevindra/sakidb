<script lang="ts">
  import { onDestroy } from 'svelte';
  import type { CellValue, ColumnDef } from '$lib/types';
  import { cellToPlainText, cellToJsonValue, copyToClipboard } from '$lib/copy-utils';
  import { cellToImageSrc } from '$lib/image-utils';

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
  const isNull = $derived(cell === 'Null');
  const isBytes = $derived(cell !== 'Null' && typeof cell === 'object' && 'Bytes' in cell);
  const imageSrc = $derived(cellToImageSrc(cell));

  onDestroy(() => {
    if (imageSrc) URL.revokeObjectURL(imageSrc);
  });

  // Position: prefer below-right of anchor, flip if needed
  const position = $derived.by(() => {
    const MARGIN = 8;
    const maxW = 400;
    const maxH = 300;

    let top = anchorRect.bottom - containerRect.top + MARGIN;
    let left = anchorRect.left - containerRect.left;

    // Flip up if not enough space below
    if (top + maxH > containerRect.height) {
      top = anchorRect.top - containerRect.top - maxH - MARGIN;
    }

    // Clamp left
    if (left + maxW > containerRect.width) {
      left = Math.max(0, containerRect.width - maxW - MARGIN);
    }

    return { top: Math.max(0, top), left: Math.max(0, left) };
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
  style="top: {position.top}px; left: {position.left}px; max-width: 400px; max-height: 300px;"
>
  <!-- Header -->
  <div class="flex items-center justify-between px-3 py-1.5 border-b border-border bg-card">
    <div class="flex items-center gap-2 text-xs">
      <span class="font-medium text-foreground">{column.name}</span>
      <span class="text-text-dim text-[10px]">{column.data_type}</span>
    </div>
    <button
      class="text-xs px-2 py-0.5 rounded text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors"
      onclick={handleCopy}
    >
      {copied ? 'Copied' : 'Copy'}
    </button>
  </div>

  <!-- Content -->
  <div class="overflow-auto p-3 text-xs" style="max-height: 256px;">
    {#if imageSrc}
      <img src={imageSrc} alt={column.name} class="max-w-full max-h-[230px] rounded object-contain" />
    {:else if isNull}
      <span class="text-text-dim italic">NULL</span>
    {:else if isJson}
      <pre class="font-mono text-primary whitespace-pre-wrap break-words">{formattedValue}</pre>
    {:else if isBytes}
      <pre class="font-mono text-text-dim whitespace-pre">{formattedValue}</pre>
    {:else}
      <div class="whitespace-pre-wrap break-words">{formattedValue}</div>
    {/if}
  </div>
</div>
