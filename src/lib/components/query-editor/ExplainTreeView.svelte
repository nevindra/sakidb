<script lang="ts">
  import type { ExplainPlan, ExplainNode } from '$lib/utils/explain-parser';
  import { ChevronRight, ChevronDown, AlertTriangle, TriangleAlert } from '@lucide/svelte';

  let { plan }: { plan: ExplainPlan } = $props();

  let collapsedKeys: Set<string> = $state(new Set());

  /**
   * Assign a stable key to each node by walking the tree depth-first.
   * Key format: "depth-preorderIndex"
   */
  function buildKeyMap(root: ExplainNode): Map<ExplainNode, string> {
    const map = new Map<ExplainNode, string>();
    let counter = 0;
    function walk(node: ExplainNode) {
      map.set(node, `${node.depth}-${counter++}`);
      for (const child of node.children) {
        walk(child);
      }
    }
    walk(root);
    return map;
  }

  let keyMap = $derived(buildKeyMap(plan.nodes));

  function getKey(node: ExplainNode): string {
    return keyMap.get(node) ?? `fallback-${node.depth}`;
  }

  function toggleCollapse(key: string) {
    const next = new Set(collapsedKeys);
    if (next.has(key)) {
      next.delete(key);
    } else {
      next.add(key);
    }
    collapsedKeys = next;
  }

  function borderColor(pct: number | undefined): string {
    if (pct === undefined) return 'border-muted';
    if (pct > 50) return 'border-red-400';
    if (pct >= 20) return 'border-amber-400';
    return 'border-emerald-400';
  }

  function formatNumber(n: number | undefined): string {
    if (n === undefined) return '';
    return n.toLocaleString();
  }
</script>

{#snippet nodeCard(node: ExplainNode)}
  {@const key = getKey(node)}
  {@const collapsed = collapsedKeys.has(key)}
  <div class="rounded-md bg-muted/20 border-l-2 {borderColor(node.percentOfTotal)} text-xs">
    <!-- Header -->
    <button
      class="flex items-center w-full gap-2 p-2 text-left hover:bg-accent/20 transition-colors"
      onclick={() => toggleCollapse(key)}
    >
      {#if node.children.length > 0}
        {#if collapsed}
          <ChevronRight class="h-3 w-3 text-muted-foreground shrink-0" />
        {:else}
          <ChevronDown class="h-3 w-3 text-muted-foreground shrink-0" />
        {/if}
      {:else}
        <span class="inline-block w-3 shrink-0"></span>
      {/if}

      <span class="font-semibold text-foreground">{node.nodeType}</span>

      {#if node.relation}
        <span class="text-muted-foreground">{node.relation}</span>
      {/if}

      <!-- Badges -->
      {#if node.rowEstimateFactor !== undefined && node.rowEstimateFactor > 10}
        <span class="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded text-[10px] font-medium bg-red-400/20 text-red-400">
          <AlertTriangle class="h-2.5 w-2.5" />
          {node.rowEstimateFactor.toFixed(0)}x misestimate
        </span>
      {/if}

      {#if node.nodeType.includes('Seq Scan') && node.actualRows !== undefined && node.actualRows > 1000}
        <span class="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded text-[10px] font-medium bg-amber-400/20 text-amber-400">
          <TriangleAlert class="h-2.5 w-2.5" />
          Sequential scan
        </span>
      {/if}

      <span class="flex-1"></span>

      {#if node.exclusiveTimeMs !== undefined}
        <span class="font-mono text-muted-foreground shrink-0">{node.exclusiveTimeMs.toFixed(1)}ms</span>
      {/if}
    </button>

    <!-- Subtitle and details -->
    {#if !collapsed}
      <div class="px-2 pb-2 pl-7 space-y-1">
        <!-- Row estimates -->
        <div class="flex items-center gap-3 text-muted-foreground">
          <span>Est. {formatNumber(node.planRows)} rows</span>
          {#if plan.isAnalyze && node.actualRows !== undefined}
            <span>Act. {formatNumber(node.actualRows)} rows</span>
          {/if}
        </div>

        <!-- Detail conditions -->
        {#if node.filter}
          <div class="text-muted-foreground">
            <span class="text-foreground/60">Filter:</span> <span class="font-mono">{node.filter}</span>
          </div>
        {/if}
        {#if node.indexCond}
          <div class="text-muted-foreground">
            <span class="text-foreground/60">Index Cond:</span> <span class="font-mono">{node.indexCond}</span>
          </div>
        {/if}
        {#if node.joinFilter}
          <div class="text-muted-foreground">
            <span class="text-foreground/60">Join Filter:</span> <span class="font-mono">{node.joinFilter}</span>
          </div>
        {/if}
        {#if node.hashCond}
          <div class="text-muted-foreground">
            <span class="text-foreground/60">Hash Cond:</span> <span class="font-mono">{node.hashCond}</span>
          </div>
        {/if}
        {#if node.sortKey && node.sortKey.length > 0}
          <div class="text-muted-foreground">
            <span class="text-foreground/60">Sort Key:</span> <span class="font-mono">{node.sortKey.join(', ')}</span>
          </div>
        {/if}

        <!-- Children -->
        {#if node.children.length > 0}
          <div class="ml-4 space-y-1 pt-1">
            {#each node.children as child}
              {@render nodeCard(child)}
            {/each}
          </div>
        {/if}
      </div>
    {/if}
  </div>
{/snippet}

<div class="flex flex-col h-full overflow-hidden text-xs">
  <div class="flex-1 overflow-auto p-2 space-y-1">
    {@render nodeCard(plan.nodes)}
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
