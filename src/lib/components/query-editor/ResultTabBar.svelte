<script lang="ts">
  import type { AnyQueryResult } from '$lib/types';

  let {
    results,
    activeIndex,
    totalExecutionTimeMs,
    statements,
    onselect,
  }: {
    results: AnyQueryResult[];
    activeIndex: number;
    totalExecutionTimeMs: number;
    statements: string[];
    onselect: (index: number) => void;
  } = $props();

  function getSmartLabel(sql: string, result: AnyQueryResult, index: number): string {
    const trimmed = sql.trim();

    // DML/DDL: no columns returned — use the first SQL keyword
    if (result.columns.length === 0) {
      const firstWord = trimmed.split(/\s/)[0];
      return firstWord ? firstWord.toUpperCase() : `Result ${index + 1}`;
    }

    // Try to extract table name from FROM clause
    const fromMatch = trimmed.match(/FROM\s+(?:(?:"[^"]+"|[\w]+)\.)?(?:"([^"]+)"|(\w+))/i);
    if (fromMatch) {
      return fromMatch[1] || fromMatch[2];
    }

    // Fallback: first 20 chars
    if (trimmed.length <= 20) return trimmed;
    return trimmed.slice(0, 20) + '\u2026';
  }
</script>

{#if results.length > 1}
  <div class="border-b border-border bg-card/50 shrink-0">
    <!-- Summary line -->
    <div class="px-2 py-0.5 text-[11px] text-muted-foreground">
      {results.length} statements &middot; {totalExecutionTimeMs}ms total
    </div>

    <!-- Tab row -->
    <div class="flex gap-0 overflow-x-auto px-1">
      {#each results as result, i (i)}
        {@const label = getSmartLabel(statements[i] ?? '', result, i)}
        {@const isActive = i === activeIndex}
        <button
          class="flex items-center gap-1.5 px-2.5 py-1 text-xs whitespace-nowrap transition-colors
            {isActive
              ? 'border-b-2 border-primary text-foreground'
              : 'text-muted-foreground hover:text-foreground hover:bg-accent/30'}"
          title="{statements[i] ?? ''} ({result.execution_time_ms}ms)"
          onclick={() => onselect(i)}
        >
          <span
            class="h-1.5 w-1.5 rounded-full shrink-0 {result.truncated ? 'bg-amber-400' : 'bg-emerald-400'}"
          ></span>
          {label}
        </button>
      {/each}
    </div>
  </div>
{/if}
