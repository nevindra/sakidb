<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { StructureTab, ColumnProfile } from '$lib/types';
  import { Loader2, ChevronRight, BarChart3 } from '@lucide/svelte';
  import { Button } from '$lib/components/ui/button';
  import BarChart from './BarChart.svelte';

  let { tab }: { tab: StructureTab } = $props();

  const app = getAppState();

  let expandedColumn = $state<string | null>(null);

  function toggleColumn(name: string) {
    expandedColumn = expandedColumn === name ? null : name;
  }

  function runProfiling() {
    app.loadProfilingData(tab.id);
  }

  function nullPct(p: ColumnProfile): number {
    return p.total_count > 0 ? (p.null_count / p.total_count) * 100 : 0;
  }

  function formatPct(n: number): string {
    return n === 0 ? '0%' : n < 0.1 ? '<0.1%' : `${n.toFixed(1)}%`;
  }

  function formatNum(n: number): string {
    return n.toLocaleString();
  }

  type StatRow = { label: string; value: string; sub?: string };

  function statsRows(p: ColumnProfile): StatRow[] {
    const rows: StatRow[] = [
      { label: 'Count', value: formatNum(p.total_count) },
      { label: 'Null', value: formatNum(p.null_count), sub: formatPct(nullPct(p)) },
      { label: 'Not Null', value: formatNum(p.not_null_count), sub: formatPct(p.total_count > 0 ? (p.not_null_count / p.total_count) * 100 : 0) },
    ];
    if (p.zero_count > 0) rows.push({ label: 'Zero', value: formatNum(p.zero_count), sub: formatPct((p.zero_count / p.total_count) * 100) });
    if (p.nan_count > 0) rows.push({ label: 'NaN', value: formatNum(p.nan_count), sub: formatPct((p.nan_count / p.total_count) * 100) });
    rows.push(
      { label: 'Distinct', value: formatNum(p.distinct_count) },
      { label: 'Unique', value: formatNum(p.unique_count) },
    );
    if (p.distinct_count !== p.unique_count) {
      rows.push({ label: 'Repeated', value: formatNum(p.distinct_count - p.unique_count) });
    }
    if (p.min !== null) rows.push({ label: 'Min', value: p.min });
    if (p.max !== null) rows.push({ label: 'Max', value: p.max });
    if (p.avg !== null) rows.push({ label: 'Average', value: p.avg.toFixed(2) });
    if (p.median !== null) rows.push({ label: 'Median', value: p.median });
    return rows;
  }
</script>

<div class="p-3">
  {#if !tab.profilingData && !tab.isProfilingLoading}
    <!-- Empty state -->
    <div class="flex flex-col items-center justify-center py-12 gap-3">
      <BarChart3 class="h-8 w-8 text-muted-foreground/50" />
      <p class="text-xs text-muted-foreground">Profile this table to see column statistics and value distributions.</p>
      <Button variant="outline" size="sm" class="h-7 text-xs" onclick={runProfiling}>
        Run Profiling
      </Button>
    </div>
  {:else if tab.isProfilingLoading}
    <!-- Loading -->
    <div class="flex flex-col items-center justify-center py-12 gap-2">
      <Loader2 class="h-5 w-5 animate-spin text-muted-foreground" />
      <p class="text-xs text-muted-foreground">Profiling {tab.columns.length} columns...</p>
    </div>
  {:else if tab.profilingData}
    <!-- Results -->
    <div class="flex items-center justify-between mb-3">
      <span class="text-xs text-muted-foreground">{tab.profilingData.length} columns profiled</span>
      <Button variant="ghost" size="sm" class="h-6 text-xs" onclick={runProfiling}>
        Re-run
      </Button>
    </div>

    <div class="space-y-px">
      {#each tab.profilingData as profile (profile.column_name)}
        {@const expanded = expandedColumn === profile.column_name}
        {@const np = nullPct(profile)}

        <!-- Column row -->
        <button
          class="w-full flex items-center gap-3 px-2 py-1.5 text-xs rounded transition-colors {expanded ? 'bg-sidebar-accent/70' : 'hover:bg-sidebar-accent/50'}"
          onclick={() => toggleColumn(profile.column_name)}
        >
          <ChevronRight class="h-3 w-3 text-muted-foreground shrink-0 transition-transform {expanded ? 'rotate-90' : ''}" />

          <!-- Name -->
          <span class="font-medium text-foreground min-w-[120px] text-left truncate">{profile.column_name}</span>

          <!-- Type badge -->
          <span class="text-[10px] font-mono text-muted-foreground bg-sidebar-accent/50 px-1.5 py-0.5 rounded shrink-0">
            {profile.data_type}
          </span>

          <!-- Null bar -->
          <div class="flex items-center gap-1.5 min-w-[100px]">
            <div class="w-16 h-1.5 bg-border rounded-full overflow-hidden">
              <div
                class="h-full rounded-full {np > 50 ? 'bg-warning' : np > 0 ? 'bg-info' : 'bg-success'}"
                style="width: {Math.max(np > 0 ? 2 : 0, np)}%"
              ></div>
            </div>
            <span class="text-muted-foreground text-[10px] tabular-nums">{formatPct(np)} null</span>
          </div>

          <!-- Distinct -->
          <span class="text-muted-foreground ml-auto tabular-nums">{formatNum(profile.distinct_count)} distinct</span>
        </button>

        <!-- Expanded detail -->
        {#if expanded}
          <div class="ml-5 mr-2 mb-2 mt-1 flex gap-6 rounded bg-card/50 border border-border/50 p-3">
            <!-- Stats table -->
            <div class="shrink-0">
              <h4 class="text-[10px] font-medium text-muted-foreground uppercase tracking-wider mb-2">
                {profile.column_name}
              </h4>
              <table class="text-xs">
                <tbody>
                  {#each statsRows(profile) as row}
                    <tr>
                      <td class="pr-4 py-0.5 text-muted-foreground">{row.label}</td>
                      <td class="py-0.5 text-foreground tabular-nums text-right">{row.value}</td>
                      {#if row.sub}
                        <td class="py-0.5 pl-2 text-muted-foreground tabular-nums text-right">{row.sub}</td>
                      {/if}
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>

            <!-- Histogram -->
            {#if profile.histogram.length > 0}
              <div class="flex-1 min-w-0">
                <h4 class="text-[10px] font-medium text-muted-foreground uppercase tracking-wider mb-2">
                  Value Distribution
                </h4>
                <BarChart data={profile.histogram.map(h => ({ label: h.value, value: h.count }))} />
              </div>
            {:else}
              <div class="flex-1 flex items-center justify-center">
                <span class="text-xs text-muted-foreground">No value distribution data</span>
              </div>
            {/if}
          </div>
        {/if}
      {/each}
    </div>
  {/if}
</div>
