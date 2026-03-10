<script lang="ts">
  import { onDestroy } from 'svelte';
  import { detectBinaryFormat, formatBinaryLabel, getMimeType, getExtension, bytesToObjectUrl } from '$lib/binary-utils';
  import type { BinaryFormat } from '$lib/binary-utils';
  import { Download } from '@lucide/svelte';

  let {
    bytes,
    columnName = 'file',
    maxImageHeight = 230,
    pdfHeight = 360,
  }: {
    bytes: number[];
    columnName?: string;
    maxImageHeight?: number;
    pdfHeight?: number;
  } = $props();

  const format: BinaryFormat = $derived(detectBinaryFormat(bytes));
  const label = $derived(formatBinaryLabel(format, bytes.length));
  const mime = $derived(getMimeType(format));
  const ext = $derived(getExtension(format));
  const filename = $derived(`${columnName}.${ext}`);

  let currentUrl: string | null = null;

  const objectUrl = $derived.by(() => {
    if (format.kind === 'unknown') return null;
    return bytesToObjectUrl(bytes, mime);
  });

  $effect(() => {
    if (currentUrl && currentUrl !== objectUrl) {
      URL.revokeObjectURL(currentUrl);
    }
    currentUrl = objectUrl;
  });

  onDestroy(() => {
    if (currentUrl) URL.revokeObjectURL(currentUrl);
  });

  function formatHex(b: number[]): string {
    const lines: string[] = [];
    const len = Math.min(b.length, 256);
    for (let i = 0; i < len; i += 16) {
      const offset = i.toString(16).padStart(8, '0');
      const hex = b.slice(i, i + 16).map(v => v.toString(16).padStart(2, '0')).join(' ');
      lines.push(`${offset}  ${hex}`);
    }
    if (b.length > 256) lines.push(`... ${b.length - 256} more bytes`);
    return lines.join('\n');
  }

  const hexDump = $derived(format.kind === 'unknown' ? formatHex(bytes) : '');
</script>

<div class="flex flex-col gap-2">
  {#if format.kind === 'image' && objectUrl}
    <img
      src={objectUrl}
      alt={columnName}
      class="max-w-full rounded object-contain"
      style="max-height: {maxImageHeight}px;"
    />
  {:else if format.kind === 'pdf' && objectUrl}
    <iframe
      src={objectUrl}
      title="{columnName} PDF"
      class="w-full rounded border border-border/40 bg-white"
      style="height: {pdfHeight}px;"
    ></iframe>
  {:else if format.kind === 'archive'}
    <div class="flex items-center gap-3 p-4 rounded-md bg-accent/10 border border-border/40">
      <div class="flex items-center justify-center w-10 h-10 rounded-md bg-accent/20 text-lg shrink-0">
        {#if format.format === 'zip'}
          <span>📦</span>
        {:else}
          <span>🗜️</span>
        {/if}
      </div>
      <div class="min-w-0">
        <div class="text-xs font-medium text-foreground truncate">{filename}</div>
        <div class="text-[10px] text-muted-foreground">{label}</div>
      </div>
    </div>
  {:else}
    <pre class="font-mono text-text-dim text-xs whitespace-pre overflow-x-auto">{hexDump}</pre>
  {/if}

  {#if format.kind !== 'unknown' && objectUrl}
    <div class="flex items-center justify-between">
      <span class="text-[10px] text-muted-foreground">{label}</span>
      <a
        href={objectUrl}
        download={filename}
        class="inline-flex items-center gap-1 text-[11px] px-2 py-1 rounded text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors"
      >
        <Download class="h-3 w-3" />
        Download
      </a>
    </div>
  {/if}
</div>
