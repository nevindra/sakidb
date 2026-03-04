<script lang="ts">
  import type { ExplainPlan, ExplainNode } from '$lib/utils/explain-parser';
  import { ChevronRight, ChevronDown } from '@lucide/svelte';

  let { plan }: { plan: ExplainPlan } = $props();

  interface FlatRow {
    node: ExplainNode;
    depth: number;
    index: number;
    key: string;
  }

  let collapsedKeys: Set<string> = $state(new Set());

  function flattenTree(node: ExplainNode, depth: number, index: { value: number }): FlatRow[] {
    const key = `${depth}-${index.value}`;
    const row: FlatRow = { node, depth, index: index.value, key };
    index.value++;
    const rows: FlatRow[] = [row];

    if (!collapsedKeys.has(key)) {
      for (const child of node.children) {
        rows.push(...flattenTree(child, depth + 1, index));
      }
    }

    return rows;
  }

  let flatRows = $derived(flattenTree(plan.nodes, 0, { value: 0 }));

  function toggleCollapse(key: string) {
    const next = new Set(collapsedKeys);
    if (next.has(key)) {
      next.delete(key);
    } else {
      next.add(key);
    }
    collapsedKeys = next;
  }

  function formatNumber(n: number | undefined): string {
    if (n === undefined) return '';
    return n.toLocaleString();
  }

  function percentBarColor(pct: number): string {
    if (pct > 50) return 'bg-red-400';
    if (pct >= 20) return 'bg-amber-400';
    return 'bg-emerald-400';
  }

  function actualRowsColor(factor: number | undefined): string {
    if (factor === undefined) return '';
    if (factor > 10) return 'text-red-400';
    if (factor > 3) return 'text-amber-400';
    return '';
  }
</script>

<div class="flex flex-col h-full overflow-hidden text-xs">
  <div class="flex-1 overflow-auto">
    <table class="w-full border-collapse">
      <thead class="sticky top-0 z-10 bg-card">
        <tr class="border-b border-border text-left text-muted-foreground">
          <th class="px-2 py-1.5 font-medium">Node Type</th>
          <th class="px-2 py-1.5 font-medium">Relation</th>
          <th class="px-2 py-1.5 font-medium text-right">Est. Rows</th>
          {#if plan.isAnalyze}
            <th class="px-2 py-1.5 font-medium text-right">Act. Rows</th>
            <th class="px-2 py-1.5 font-medium text-right">Time</th>
            <th class="px-2 py-1.5 font-medium w-32">% of Total</th>
          {/if}
        </tr>
      </thead>
      <tbody>
        {#each flatRows as row (row.key)}
          <tr class="border-b border-border/50 hover:bg-accent/20 transition-colors">
            <!-- Node Type -->
            <td class="px-2 py-1" style="padding-left: {row.depth * 16 + 8}px">
              <span class="inline-flex items-center gap-1">
                {#if row.node.children.length > 0}
                  <button
                    class="shrink-0 text-muted-foreground hover:text-foreground transition-colors"
                    onclick={() => toggleCollapse(row.key)}
                  >
                    {#if collapsedKeys.has(row.key)}
                      <ChevronRight class="h-3 w-3" />
                    {:else}
                      <ChevronDown class="h-3 w-3" />
                    {/if}
                  </button>
                {:else}
                  <span class="inline-block w-3 shrink-0"></span>
                {/if}
                <span class="text-foreground">{row.node.nodeType}</span>
              </span>
            </td>

            <!-- Relation -->
            <td class="px-2 py-1 text-muted-foreground">{row.node.relation ?? ''}</td>

            <!-- Est. Rows -->
            <td class="px-2 py-1 text-right font-mono text-muted-foreground">
              {formatNumber(row.node.planRows)}
            </td>

            {#if plan.isAnalyze}
              <!-- Act. Rows -->
              <td class="px-2 py-1 text-right font-mono {actualRowsColor(row.node.rowEstimateFactor)}">
                {formatNumber(row.node.actualRows)}
              </td>

              <!-- Time -->
              <td class="px-2 py-1 text-right font-mono text-muted-foreground">
                {#if row.node.exclusiveTimeMs !== undefined}
                  {row.node.exclusiveTimeMs.toFixed(1)}ms
                {/if}
              </td>

              <!-- % of Total -->
              <td class="px-2 py-1">
                {#if row.node.percentOfTotal !== undefined}
                  <div class="flex items-center gap-1.5">
                    <div class="flex-1 h-1.5 rounded-full bg-muted/30 overflow-hidden">
                      <div
                        class="h-full rounded-full {percentBarColor(row.node.percentOfTotal)}"
                        style="width: {Math.min(row.node.percentOfTotal, 100)}%"
                      ></div>
                    </div>
                    <span class="text-muted-foreground font-mono w-10 text-right shrink-0">
                      {row.node.percentOfTotal.toFixed(1)}%
                    </span>
                  </div>
                {/if}
              </td>
            {/if}
          </tr>
        {/each}
      </tbody>
    </table>
  </div>

  <!-- Footer -->
  {#if plan.planningTimeMs !== undefined || plan.executionTimeMs !== undefined}
    <div class="flex items-center gap-4 px-2 py-1.5 border-t border-border bg-card text-muted-foreground shrink-0">
      {#if plan.planningTimeMs !== undefined}
        <span>Planning Time: <span class="font-mono text-foreground">{plan.planningTimeMs.toFixed(2)} ms</span></span>
      {/if}
      {#if plan.executionTimeMs !== undefined}
        <span>Execution Time: <span class="font-mono text-foreground">{plan.executionTimeMs.toFixed(2)} ms</span></span>
      {/if}
    </div>
  {/if}
</div>
