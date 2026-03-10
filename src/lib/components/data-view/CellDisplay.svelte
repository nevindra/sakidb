<script lang="ts">
  import { onDestroy } from 'svelte';
  import type { CellValue } from '$lib/types';
  import { detectBinaryFormat, formatBinaryLabel, getMimeType, bytesToObjectUrl } from '$lib/binary-utils';
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
      const format = detectBinaryFormat(v.Bytes);
      if (format.kind !== 'unknown') {
        return { text: formatBinaryLabel(format, v.Bytes.length), cls: 'text-text-dim text-[10px]' };
      }
      const hex = v.Bytes.slice(0, 16).map(b => b.toString(16).padStart(2, '0')).join(' ');
      return { text: `\\x${hex}${v.Bytes.length > 16 ? '...' : ''}`, cls: 'font-mono text-text-dim' };
    }
    return { text: '?', cls: '' };
  }

  function truncate(s: string): string {
    return s.length > MAX_CELL_LEN ? s.slice(0, MAX_CELL_LEN) + '…' : s;
  }

  const isBytes = $derived(value !== 'Null' && typeof value === 'object' && 'Bytes' in value);
  const bytesFormat = $derived(isBytes ? detectBinaryFormat((value as { Bytes: number[] }).Bytes) : null);
  const isImage = $derived(bytesFormat?.kind === 'image');

  let currentObjectUrl: string | null = null;

  const imageSrc = $derived.by(() => {
    if (!isImage || !bytesFormat || bytesFormat.kind !== 'image') return null;
    const bytes = (value as { Bytes: number[] }).Bytes;
    return bytesToObjectUrl(bytes, getMimeType(bytesFormat));
  });

  $effect(() => {
    if (currentObjectUrl && currentObjectUrl !== imageSrc) {
      URL.revokeObjectURL(currentObjectUrl);
    }
    currentObjectUrl = imageSrc;
  });

  onDestroy(() => {
    if (currentObjectUrl) URL.revokeObjectURL(currentObjectUrl);
  });

  const display = $derived(getDisplay(value));
</script>

{#if isImage && imageSrc}
  <span class="inline-flex items-center gap-1.5">
    <img src={imageSrc} alt="" class="h-5 w-5 object-cover rounded-sm shrink-0" />
    <span class="text-text-dim text-[10px]">{display.text}</span>
  </span>
{:else if isBytes && bytesFormat && bytesFormat.kind === 'pdf'}
  <span class="inline-flex items-center gap-1.5">
    <span class="text-red-400/80 text-[10px] shrink-0">PDF</span>
    <span class="text-text-dim text-[10px]">{display.text}</span>
  </span>
{:else if isBytes && bytesFormat && bytesFormat.kind === 'archive'}
  <span class="inline-flex items-center gap-1.5">
    <span class="text-amber-400/80 text-[10px] shrink-0">📦</span>
    <span class="text-text-dim text-[10px]">{display.text}</span>
  </span>
{:else}
  <span class={display.cls}>{display.text}</span>
{/if}
