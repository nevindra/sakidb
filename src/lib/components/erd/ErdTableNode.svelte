<script lang="ts">
  import type { ColumnInfo, TableInfo } from '$lib/types';
  import { Key, Link } from '@lucide/svelte';

  let {
    table,
    columns,
    x,
    y,
    selected = false,
    dimmed = false,
    highlightedColumn = null,
    onselect,
    ondblclick,
    onpointerdown,
    oncolumnenter,
    oncolumnleave,
  }: {
    table: TableInfo;
    columns: ColumnInfo[];
    x: number;
    y: number;
    selected?: boolean;
    dimmed?: boolean;
    highlightedColumn?: string | null;
    onselect?: () => void;
    ondblclick?: () => void;
    onpointerdown?: (e: PointerEvent) => void;
    oncolumnenter?: (colName: string) => void;
    oncolumnleave?: () => void;
  } = $props();

  // FK columns determined externally via highlight
  const fkColumns = $derived(new Set<string>());

  function formatRowCount(count: number | null): string {
    if (count === null) return '';
    if (count >= 1_000_000) return `${(count / 1_000_000).toFixed(1)}M`;
    if (count >= 1_000) return `${(count / 1_000).toFixed(1)}k`;
    return `${count}`;
  }

  function shortenType(dt: string): string {
    return dt
      .replace('character varying', 'varchar')
      .replace('timestamp without time zone', 'timestamp')
      .replace('timestamp with time zone', 'timestamptz')
      .replace('double precision', 'float8')
      .replace('boolean', 'bool')
      .replace('integer', 'int4')
      .replace('bigint', 'int8')
      .replace('smallint', 'int2');
  }
</script>

<div
  class="erd-node absolute select-none"
  class:erd-node--selected={selected}
  class:erd-node--dimmed={dimmed}
  style:left="{x}px"
  style:top="{y}px"
  role="button"
  tabindex={0}
  onclick={onselect}
  ondblclick={ondblclick}
  onpointerdown={onpointerdown}
  onkeydown={(e) => { if (e.key === 'Enter') onselect?.(); }}
>
  <!-- Header -->
  <div class="erd-node__header">
    <span class="erd-node__title">{table.name}</span>
    {#if table.row_count_estimate != null}
      <span class="erd-node__badge">{formatRowCount(table.row_count_estimate)}</span>
    {/if}
  </div>

  <!-- Columns -->
  <div class="erd-node__columns">
    {#each columns as col, i (col.name)}
      <div
        class="erd-node__col"
        class:erd-node__col--highlighted={highlightedColumn === col.name}
        data-table={table.name}
        data-column={col.name}
        data-row={i}
        role="listitem"
        onpointerenter={() => oncolumnenter?.(col.name)}
        onpointerleave={() => oncolumnleave?.()}
      >
        <span class="erd-node__col-icon">
          {#if col.is_primary_key}
            <Key class="h-2.5 w-2.5 text-warning" />
          {:else if highlightedColumn === col.name}
            <Link class="h-2.5 w-2.5 text-primary" />
          {:else}
            <span class="w-2.5"></span>
          {/if}
        </span>
        <span class="erd-node__col-name">{col.name}</span>
        <span class="erd-node__col-type">{shortenType(col.data_type)}</span>
      </div>
    {/each}
  </div>
</div>

<style>
  .erd-node {
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    min-width: 180px;
    max-width: 280px;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
    cursor: grab;
    transition: box-shadow 0.15s, border-color 0.15s, opacity 0.2s;
    overflow: hidden;
  }

  .erd-node:hover {
    border-color: var(--muted-foreground);
  }

  .erd-node--selected {
    border-color: var(--primary);
    box-shadow: 0 0 0 1px var(--primary), 0 4px 16px rgba(94, 106, 210, 0.2);
  }

  .erd-node--dimmed {
    opacity: 0.3;
  }

  .erd-node__header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 10px;
    background: var(--secondary);
    border-bottom: 1px solid var(--border);
  }

  .erd-node__title {
    font-size: 11px;
    font-weight: 600;
    color: var(--foreground);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .erd-node__badge {
    font-size: 9px;
    color: var(--muted-foreground);
    background: var(--muted);
    padding: 1px 5px;
    border-radius: 9999px;
    white-space: nowrap;
    margin-left: 6px;
  }

  .erd-node__columns {
    padding: 2px 0;
  }

  .erd-node__col {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 1.5px 10px;
    font-size: 11px;
    transition: background 0.1s;
  }

  .erd-node__col:hover {
    background: var(--sidebar-accent);
  }

  .erd-node__col--highlighted {
    background: color-mix(in srgb, var(--primary) 15%, transparent);
  }

  .erd-node__col-icon {
    display: flex;
    align-items: center;
    width: 10px;
    flex-shrink: 0;
  }

  .erd-node__col-name {
    color: var(--foreground);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
  }

  .erd-node__col-type {
    color: var(--muted-foreground);
    font-family: monospace;
    font-size: 10px;
    white-space: nowrap;
    margin-left: auto;
    padding-left: 8px;
  }
</style>
