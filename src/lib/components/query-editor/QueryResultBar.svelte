<script lang="ts">
  import { save } from '@tauri-apps/plugin-dialog';
  import { writeTextFile } from '@tauri-apps/plugin-fs';
  import type { AnyQueryResult } from '$lib/types';
  import { CheckCircle2, Info, AlertTriangle, ClipboardCopy, Download, Columns2, Check } from '@lucide/svelte';
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
  import * as Tooltip from '$lib/components/ui/tooltip';
  import { resultToDelimited, resultToJson } from '$lib/utils/result-diff';

  let {
    result,
    resultIndex,
    totalResults,
    totalExecutionTimeMs,
    oncompare,
  }: {
    result: AnyQueryResult;
    resultIndex: number;
    totalResults: number;
    totalExecutionTimeMs: number;
    oncompare?: () => void;
  } = $props();

  function formatNumber(n: number): string {
    return n.toLocaleString();
  }

  let hasColumns = $derived(result.columns.length > 0);
  let copied = $state(false);

  async function handleCopy() {
    const tsv = resultToDelimited(result, '\t');
    await navigator.clipboard.writeText(tsv);
    copied = true;
    setTimeout(() => { copied = false; }, 2000);
  }

  async function handleExport(format: 'csv' | 'json') {
    const ext = format;
    const filePath = await save({
      defaultPath: `query-results.${ext}`,
      filters: [
        format === 'csv'
          ? { name: 'CSV', extensions: ['csv'] }
          : { name: 'JSON', extensions: ['json'] },
      ],
    });

    if (!filePath) return;

    const content = format === 'csv'
      ? resultToDelimited(result, ',')
      : resultToJson(result);

    await writeTextFile(filePath, content);
  }
</script>

<div class="flex items-center h-7 px-2 gap-2 text-xs bg-card border-t border-border shrink-0">
  <!-- Left side: status info -->
  {#if hasColumns}
    <CheckCircle2 class="h-3 w-3 text-emerald-400 shrink-0" />
    <span class="text-muted-foreground">{formatNumber(result.row_count)} rows</span>
    <span class="text-muted-foreground/40 select-none">&middot;</span>
    <span class="text-muted-foreground">{result.columns.length} columns</span>
  {:else}
    <Info class="h-3 w-3 text-blue-400 shrink-0" />
    <span class="text-muted-foreground">{result.row_count} rows affected</span>
  {/if}

  <span class="text-muted-foreground/40 select-none">&middot;</span>

  <span class="text-muted-foreground font-mono">{result.execution_time_ms}ms</span>

  <!-- Spacer -->
  <div class="flex-1"></div>

  <!-- Right side: actions + info -->
  {#if result.truncated}
    <div class="flex items-center gap-1 text-amber-400">
      <AlertTriangle class="h-3 w-3 shrink-0" />
      <span>Truncated at 500K rows</span>
    </div>
  {/if}

  {#if hasColumns}
    <!-- Copy to clipboard -->
    <Tooltip.Root>
      <Tooltip.Trigger>
        <button
          class="p-0.5 rounded text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors"
          onclick={handleCopy}
        >
          {#if copied}
            <Check class="h-3.5 w-3.5 text-emerald-400" />
          {:else}
            <ClipboardCopy class="h-3.5 w-3.5" />
          {/if}
        </button>
      </Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content side="top" class="text-xs">
          {copied ? 'Copied!' : 'Copy as TSV'}
        </Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>

    <!-- Export dropdown -->
    <DropdownMenu.Root>
      <Tooltip.Root>
        <Tooltip.Trigger>
          <DropdownMenu.Trigger>
            <button class="p-0.5 rounded text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors">
              <Download class="h-3.5 w-3.5" />
            </button>
          </DropdownMenu.Trigger>
        </Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Content side="top" class="text-xs">Export results</Tooltip.Content>
        </Tooltip.Portal>
      </Tooltip.Root>
      <DropdownMenu.Portal>
        <DropdownMenu.Content align="end" class="min-w-[140px]">
          <DropdownMenu.Item onclick={() => handleExport('csv')}>Export as CSV</DropdownMenu.Item>
          <DropdownMenu.Item onclick={() => handleExport('json')}>Export as JSON</DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>

    <!-- Compare (only for multi-result) -->
    {#if totalResults > 1 && oncompare}
      <Tooltip.Root>
        <Tooltip.Trigger>
          <button
            class="p-0.5 rounded text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors"
            onclick={oncompare}
          >
            <Columns2 class="h-3.5 w-3.5" />
          </button>
        </Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Content side="top" class="text-xs">Compare results</Tooltip.Content>
        </Tooltip.Portal>
      </Tooltip.Root>
    {/if}
  {/if}

  {#if totalResults > 1}
    <span class="text-muted-foreground">Result {resultIndex + 1} of {totalResults}</span>
  {/if}
</div>
