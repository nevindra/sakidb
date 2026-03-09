<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { getAppState } from '$lib/stores';
  import type { DataTab, ColumnInfo } from '$lib/types';
  import DataGrid from './DataGrid.svelte';
  import { Button } from '$lib/components/ui/button';
  import { Loader2, Square } from '@lucide/svelte';

  let { tab }: { tab: DataTab } = $props();

  const app = getAppState();

  // ── Column info for editing (PK detection) ──
  let columnInfos = $state<ColumnInfo[]>([]);

  // Fetch column info when tab changes
  $effect(() => {
    const connId = tab.runtimeConnectionId;
    const schema = tab.schema;
    const table = tab.table;
    if (connId && table) {
      invoke<ColumnInfo[]>('list_columns', {
        activeConnectionId: connId,
        schema,
        table,
      }).then(cols => {
        columnInfos = cols;
      }).catch(() => {
        columnInfos = [];
      });
    }
  });

  function reloadData() {
    app.loadDataTab(tab.id, tab.currentPage);
  }

  // ── Elapsed timer ──
  let elapsedMs = $state(0);
  let timerInterval: ReturnType<typeof setInterval> | null = null;
  let startTime: number | null = null;

  $effect(() => {
    if (tab.isLoading) {
      startTime = Date.now();
      elapsedMs = 0;
      timerInterval = setInterval(() => {
        elapsedMs = Date.now() - (startTime ?? Date.now());
      }, 100);
    } else {
      if (timerInterval) {
        clearInterval(timerInterval);
        timerInterval = null;
      }
      startTime = null;
      elapsedMs = 0;
    }

    return () => {
      if (timerInterval) {
        clearInterval(timerInterval);
        timerInterval = null;
      }
    };
  });

  function formatElapsed(ms: number): string {
    const totalSec = ms / 1000;
    if (totalSec < 60) {
      return `${totalSec.toFixed(1)}s`;
    }
    const min = Math.floor(totalSec / 60);
    const sec = Math.floor(totalSec % 60);
    return `${min}m ${sec}s`;
  }
</script>

<div class="flex flex-col h-full bg-background min-w-0">
  {#if tab.isLoading && !tab.queryResult}
    <!-- Initial loading -->
    <div class="flex-1 flex items-center justify-center">
      <div class="flex flex-col items-center gap-3">
        <div class="flex items-center gap-2 text-muted-foreground text-sm">
          <Loader2 class="h-4 w-4 animate-spin" />
          Loading {tab.schema}.{tab.table}...
          <span class="tabular-nums text-warning font-medium">{formatElapsed(elapsedMs)}</span>
        </div>
        <Button
          size="sm"
          variant="destructive"
          class="h-7 text-xs"
          onclick={() => app.cancelQuery(tab.id)}
        >
          <Square class="h-3 w-3 mr-1" />
          Cancel
        </Button>
      </div>
    </div>
  {:else if tab.queryResult}
    <!-- Data grid with loading overlay -->
    <div class="flex-1 overflow-hidden relative min-w-0">
      {#if tab.isLoading}
        <div class="absolute inset-0 bg-background/50 z-20 flex items-center justify-center">
          <div class="flex flex-col items-center gap-3">
            <div class="flex items-center gap-2">
              <Loader2 class="h-4 w-4 animate-spin text-primary" />
              <span class="tabular-nums text-warning text-sm font-medium">{formatElapsed(elapsedMs)}</span>
            </div>
            <Button
              size="sm"
              variant="destructive"
              class="h-7 text-xs"
              onclick={() => app.cancelQuery(tab.id)}
            >
              <Square class="h-3 w-3 mr-1" />
              Cancel
            </Button>
          </div>
        </div>
      {/if}
      <DataGrid
        result={tab.queryResult}
        tabId={tab.id}
        schema={tab.schema}
        table={tab.table}
        connectionId={tab.runtimeConnectionId}
        savedConnectionId={tab.savedConnectionId}
        databaseName={tab.databaseName}
        {columnInfos}
        filters={tab.filters}
        currentPage={tab.currentPage}
        pageSize={tab.pageSize}
        totalRowEstimate={tab.totalRowEstimate}
        onreload={reloadData}
      />
    </div>
  {:else}
    <div class="flex-1 flex items-center justify-center text-muted-foreground text-sm">
      Failed to load data
    </div>
  {/if}
</div>
