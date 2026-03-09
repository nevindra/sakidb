<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { AnyQueryResult } from '$lib/types';
  import { ChevronLeft, ChevronRight, Timer, LockKeyhole } from '@lucide/svelte';
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu';

  let {
    result,
    tabId,
    currentPage,
    pageSize,
    totalPages,
    canEdit,
    schema,
    pendingInsertCount,
    pendingDeleteCount,
  }: {
    result: AnyQueryResult;
    tabId: string;
    currentPage: number;
    pageSize: number;
    totalPages: number;
    canEdit: boolean;
    schema: string;
    pendingInsertCount: number;
    pendingDeleteCount: number;
  } = $props();

  const app = getAppState();

  const PAGE_SIZE_OPTIONS = [50, 100, 500, 1000];

  const TIMEOUT_PRESETS = [
    { label: 'No limit', value: null },
    { label: '15s', value: 15 },
    { label: '30s', value: 30 },
    { label: '60s', value: 60 },
    { label: '2m', value: 120 },
    { label: '5m', value: 300 },
  ] as const;

  function formatTimeoutLabel(seconds: number | null): string {
    if (seconds === null) return 'No limit';
    if (seconds < 60) return `${seconds}s`;
    return `${seconds / 60}m`;
  }

  let editingPage = $state(false);
  let pageInputValue = $state('');

  function goToPage(page: number) {
    app.loadDataTab(tabId, page);
  }

  function changePageSize(newSize: number) {
    app.updateDataTabPageSize(tabId, newSize);
  }

  function focusOnMount(node: HTMLInputElement) {
    node.focus();
    node.select();
  }

  function startPageEdit() {
    pageInputValue = String(currentPage + 1);
    editingPage = true;
  }

  function commitPageJump() {
    editingPage = false;
    const parsed = parseInt(pageInputValue, 10);
    if (!isNaN(parsed) && parsed >= 1 && parsed <= totalPages) {
      goToPage(parsed - 1);
    }
  }

  function cancelPageEdit() {
    editingPage = false;
  }
</script>

<div class="px-2 py-1 border-t border-border flex items-center gap-3 text-[11px] text-muted-foreground bg-card shrink-0">
  <!-- Row count + execution time -->
  <span class="tabular-nums shrink-0">
    {#if pendingInsertCount > 0}
      <span class="text-success">+{pendingInsertCount}</span>
    {/if}
    {#if pendingDeleteCount > 0}
      <span class="text-destructive">-{pendingDeleteCount}</span>
    {/if}
  </span>
  <span class="text-text-dim tabular-nums shrink-0">{result.execution_time_ms}ms</span>

  {#if !canEdit && schema !== ''}
    <span class="flex items-center gap-1 bg-warning/15 text-warning rounded px-1.5 py-0.5 shrink-0" title="No primary key — editing disabled">
      <LockKeyhole class="h-3 w-3" />
      <span>Read-only — no PK</span>
    </span>
  {/if}

  <div class="flex-1"></div>

  <!-- Page size selector -->
  <DropdownMenu.Root>
    <DropdownMenu.Trigger>
      <button class="flex items-center gap-1 hover:text-foreground transition-colors px-1 py-0.5 rounded hover:bg-accent tabular-nums">
        {pageSize} rows
      </button>
    </DropdownMenu.Trigger>
    <DropdownMenu.Content align="center" class="min-w-[100px]">
      <DropdownMenu.Label>Page size</DropdownMenu.Label>
      <DropdownMenu.Separator />
      {#each PAGE_SIZE_OPTIONS as size}
        <DropdownMenu.Item onclick={() => changePageSize(size)}>
          <span class="flex items-center justify-between w-full">
            {size}
            {#if pageSize === size}
              <span class="text-primary text-xs">✓</span>
            {/if}
          </span>
        </DropdownMenu.Item>
      {/each}
    </DropdownMenu.Content>
  </DropdownMenu.Root>

  <!-- Pagination -->
  <div class="flex items-center gap-1 shrink-0">
    <button
      class="p-0.5 rounded hover:bg-accent transition-colors disabled:opacity-30"
      disabled={currentPage === 0}
      onclick={() => goToPage(currentPage - 1)}
    >
      <ChevronLeft class="h-3 w-3" />
    </button>
    {#if editingPage}
      <input
        class="tabular-nums w-12 text-center text-xs bg-accent border border-border rounded px-1 py-0 h-5 outline-none focus:border-primary"
        type="text"
        bind:value={pageInputValue}
        onblur={commitPageJump}
        onkeydown={(e: KeyboardEvent) => {
          if (e.key === 'Enter') { e.preventDefault(); commitPageJump(); }
          if (e.key === 'Escape') { e.preventDefault(); cancelPageEdit(); }
        }}
        use:focusOnMount
      />
      <span class="text-text-dim tabular-nums">/{totalPages}</span>
    {:else}
      <button
        class="tabular-nums px-1 hover:bg-accent rounded transition-colors cursor-text"
        onclick={startPageEdit}
        title="Click to jump to page"
      >
        {currentPage + 1}<span class="text-text-dim">/{totalPages}</span>
      </button>
    {/if}
    <button
      class="p-0.5 rounded hover:bg-accent transition-colors disabled:opacity-30"
      disabled={currentPage >= totalPages - 1}
      onclick={() => goToPage(currentPage + 1)}
    >
      <ChevronRight class="h-3 w-3" />
    </button>
  </div>

  <!-- Query timeout -->
  <DropdownMenu.Root>
    <DropdownMenu.Trigger>
      <button class="flex items-center gap-1 hover:text-foreground transition-colors px-1 py-0.5 rounded hover:bg-accent" title="Query timeout">
        <Timer class="h-3 w-3" />
        <span class="text-[10px]">{formatTimeoutLabel(app.queryTimeoutSeconds)}</span>
      </button>
    </DropdownMenu.Trigger>
    <DropdownMenu.Content align="end" class="min-w-[120px]">
      <DropdownMenu.Label>Query timeout</DropdownMenu.Label>
      <DropdownMenu.Separator />
      {#each TIMEOUT_PRESETS as preset}
        <DropdownMenu.Item onclick={() => app.setQueryTimeout(preset.value)}>
          <span class="flex items-center justify-between w-full">
            {preset.label}
            {#if app.queryTimeoutSeconds === preset.value}
              <span class="text-primary text-xs">✓</span>
            {/if}
          </span>
        </DropdownMenu.Item>
      {/each}
    </DropdownMenu.Content>
  </DropdownMenu.Root>
</div>
