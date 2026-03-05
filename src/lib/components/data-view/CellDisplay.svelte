<script lang="ts">
  import { onDestroy } from 'svelte';
  import type { CellValue } from '$lib/types';
  import { cellToImageSrc } from '$lib/image-utils';
  import { getCategoryCss } from '$lib/type-utils';

  const MAX_CELL_LEN = 200;

  let { value, dataType = '' }: { value: CellValue; dataType?: string } = $props();

  function getDisplay(v: CellValue): { text: string; cls: string } {
    if (v === 'Null') return { text: 'NULL', cls: 'text-text-dim italic' };
    if ('Bool' in v) return { text: String(v.Bool), cls: 'text-warning' };
    if ('Int' in v) return { text: String(v.Int), cls: 'text-right tabular-nums' };
    if ('Float' in v) return { text: String(v.Float), cls: 'text-right tabular-nums' };
    if ('Text' in v) return { text: truncate(v.Text), cls: dataType ? getCategoryCss(dataType) : '' };
    if ('Json' in v) return { text: truncate(v.Json), cls: 'font-mono text-primary' };
    if ('Timestamp' in v) return { text: v.Timestamp, cls: 'text-success' };
    if ('Bytes' in v) {
      const hex = v.Bytes.slice(0, 16).map(b => b.toString(16).padStart(2, '0')).join(' ');
      return { text: `\\x${hex}${v.Bytes.length > 16 ? '...' : ''}`, cls: 'font-mono text-text-dim' };
    }
    return { text: '?', cls: '' };
  }

  function truncate(s: string): string {
    return s.length > MAX_CELL_LEN ? s.slice(0, MAX_CELL_LEN) + '…' : s;
  }

  let currentObjectUrl: string | null = null;
  const imageSrc = $derived(cellToImageSrc(value));
  const display = $derived(getDisplay(value));

  $effect(() => {
    if (currentObjectUrl && currentObjectUrl !== imageSrc) {
      URL.revokeObjectURL(currentObjectUrl);
    }
    currentObjectUrl = imageSrc;
  });

  onDestroy(() => {
    if (currentObjectUrl) {
      URL.revokeObjectURL(currentObjectUrl);
    }
  });
</script>

{#if imageSrc}
  <span class="inline-flex items-center gap-1.5">
    <img src={imageSrc} alt="" class="h-5 w-5 object-cover rounded-sm shrink-0" />
    <span class="text-text-dim text-[10px]">image</span>
  </span>
{:else}
  <span class={display.cls}>{display.text}</span>
{/if}
