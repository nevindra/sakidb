<script lang="ts">
  import type { AnyQueryResult } from '$lib/types';
  import { CheckCircle2, Info, AlertTriangle } from '@lucide/svelte';

  let {
    result,
    resultIndex,
    totalResults,
    totalExecutionTimeMs,
  }: {
    result: AnyQueryResult;
    resultIndex: number;
    totalResults: number;
    totalExecutionTimeMs: number;
  } = $props();

  function formatNumber(n: number): string {
    return n.toLocaleString();
  }

  let hasColumns = $derived(result.columns.length > 0);
</script>

<div class="flex items-center h-7 px-2 gap-2 text-xs bg-card border-t border-border shrink-0">
  <!-- Left side -->
  {#if hasColumns}
    <CheckCircle2 class="h-3 w-3 text-emerald-400 shrink-0" />
    <span class="text-muted-foreground">{formatNumber(result.row_count)} rows</span>
  {:else}
    <Info class="h-3 w-3 text-blue-400 shrink-0" />
    <span class="text-muted-foreground">{result.row_count} rows affected</span>
  {/if}

  <span class="text-muted-foreground/40 select-none">&middot;</span>

  <span class="text-muted-foreground font-mono">{result.execution_time_ms}ms</span>

  <!-- Spacer -->
  <div class="flex-1"></div>

  <!-- Right side -->
  {#if result.truncated}
    <div class="flex items-center gap-1 text-amber-400">
      <AlertTriangle class="h-3 w-3 shrink-0" />
      <span>Truncated at 500K rows</span>
    </div>
  {/if}

  {#if totalResults > 1}
    <span class="text-muted-foreground">Result {resultIndex + 1} of {totalResults}</span>
  {/if}
</div>
