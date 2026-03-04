<script lang="ts">
  import { onMount, tick } from 'svelte';
  import type { TableInfo, ColumnInfo, ForeignKeyInfo } from '$lib/types';
  import dagre from '@dagrejs/dagre';
  import ErdTableNode from './ErdTableNode.svelte';
  import ErdMinimap from './ErdMinimap.svelte';

  let {
    tables,
    columns,
    foreignKeys,
    focusTable = null,
    searchQuery = '',
    simplified = false,
    onopentable,
  }: {
    tables: TableInfo[];
    columns: Record<string, ColumnInfo[]>;
    foreignKeys: Record<string, ForeignKeyInfo[]>;
    focusTable?: string | null;
    searchQuery?: string;
    simplified?: boolean;
    onopentable?: (table: string) => void;
  } = $props();

  // Canvas state
  let panX = $state(0);
  let panY = $state(0);
  let zoom = $state(1);
  let containerEl: HTMLDivElement | undefined = $state();
  let canvasEl: HTMLDivElement | undefined = $state();

  // Node positions (table name → {x, y, width, height})
  let nodePositions = $state<Record<string, { x: number; y: number; width: number; height: number }>>({});
  let layoutDone = $state(false);

  // Interaction state
  let selectedTable = $state<string | null>(null);
  let highlightedFK = $state<{ table: string; column: string } | null>(null);
  let draggingNode = $state<string | null>(null);
  let dragOffset = $state({ x: 0, y: 0 });
  let isPanning = $state(false);
  let panStart = $state({ x: 0, y: 0, panX: 0, panY: 0 });


  // In simplified mode: filter out partition children, aggregate row counts into parents
  const displayTables = $derived.by(() => {
    if (!simplified) return tables;

    // Collect row counts from partitions to add to their parents
    const partitionCounts = new Map<string, number>();
    for (const t of tables) {
      if (t.is_partition && t.parent_table && t.row_count_estimate != null) {
        partitionCounts.set(
          t.parent_table,
          (partitionCounts.get(t.parent_table) ?? 0) + t.row_count_estimate,
        );
      }
    }

    return tables
      .filter(t => !t.is_partition)
      .map(t => {
        const extra = partitionCounts.get(t.name);
        if (extra == null) return t;
        return {
          ...t,
          row_count_estimate: (t.row_count_estimate ?? 0) + extra,
        };
      });
  });

  // Resolved edges from the keyed FK data — no guessing needed
  const resolvedEdges = $derived.by(() => {
    const edges: { id: string; source: string; sourceCol: string; target: string; targetCol: string; fk: ForeignKeyInfo }[] = [];
    const tableNames = new Set(displayTables.map(t => t.name));

    for (const [sourceTable, fks] of Object.entries(foreignKeys)) {
      if (!tableNames.has(sourceTable)) continue;
      for (const fk of fks) {
        if (!tableNames.has(fk.foreign_table_name)) continue;
        edges.push({
          id: `${sourceTable}:${fk.constraint_name}`,
          source: sourceTable,
          sourceCol: fk.columns[0],
          target: fk.foreign_table_name,
          targetCol: fk.foreign_columns[0],
          fk,
        });
      }
    }
    return edges;
  });

  // Tables connected to selected table via FK
  const connectedTables = $derived.by(() => {
    if (!selectedTable) return new Set<string>();
    const connected = new Set<string>();
    connected.add(selectedTable);
    for (const edge of resolvedEdges) {
      if (edge.source === selectedTable) connected.add(edge.target);
      if (edge.target === selectedTable) connected.add(edge.source);
    }
    return connected;
  });

  // Search matching
  const searchMatches = $derived(
    searchQuery
      ? new Set(displayTables.filter(t => t.name.toLowerCase().includes(searchQuery.toLowerCase())).map(t => t.name))
      : null
  );

  // Highlighted columns from FK hover
  const highlightedColumns = $derived.by(() => {
    if (!highlightedFK) return new Map<string, Set<string>>();
    const map = new Map<string, Set<string>>();
    for (const edge of resolvedEdges) {
      if (
        (edge.source === highlightedFK.table && edge.sourceCol === highlightedFK.column) ||
        (edge.target === highlightedFK.table && edge.targetCol === highlightedFK.column)
      ) {
        if (!map.has(edge.source)) map.set(edge.source, new Set());
        if (!map.has(edge.target)) map.set(edge.target, new Set());
        map.get(edge.source)!.add(edge.sourceCol);
        map.get(edge.target)!.add(edge.targetCol);
      }
    }
    return map;
  });

  // Run dagre layout
  async function runLayout() {
    if (displayTables.length === 0) return;

    // Measure node dimensions from DOM
    await tick();
    const nodeDims: Record<string, { width: number; height: number }> = {};
    if (canvasEl) {
      const nodeEls = canvasEl.querySelectorAll('.erd-node');
      for (const el of nodeEls) {
        const tableName = (el as HTMLElement).querySelector('.erd-node__title')?.textContent;
        if (tableName) {
          nodeDims[tableName] = { width: el.clientWidth, height: el.clientHeight };
        }
      }
    }

    const g = new dagre.graphlib.Graph().setDefaultEdgeLabel(() => ({}));
    g.setGraph({
      rankdir: 'LR',
      nodesep: 40,
      ranksep: 60,
      marginx: 20,
      marginy: 20,
    });

    for (const t of displayTables) {
      g.setNode(t.name, {
        width: nodeDims[t.name]?.width ?? 200,
        height: nodeDims[t.name]?.height ?? 100,
      });
    }

    for (const e of resolvedEdges) {
      g.setEdge(e.source, e.target);
    }

    dagre.layout(g);

    // dagre positions nodes at center — convert to top-left
    const positions: Record<string, { x: number; y: number; width: number; height: number }> = {};
    for (const name of g.nodes()) {
      const node = g.node(name);
      positions[name] = {
        x: node.x - node.width / 2,
        y: node.y - node.height / 2,
        width: node.width,
        height: node.height,
      };
    }
    nodePositions = positions;
    layoutDone = true;

    // Auto-fit after layout
    await tick();
    fitToScreen();

    // Focus on specific table if requested
    if (focusTable && positions[focusTable]) {
      selectedTable = focusTable;
      centerOnTable(focusTable);
    }
  }

  // Canvas bounds
  const canvasBounds = $derived.by(() => {
    const positions = Object.values(nodePositions);
    if (positions.length === 0) return { minX: 0, minY: 0, maxX: 800, maxY: 600 };
    let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
    for (const p of positions) {
      minX = Math.min(minX, p.x);
      minY = Math.min(minY, p.y);
      maxX = Math.max(maxX, p.x + p.width);
      maxY = Math.max(maxY, p.y + p.height);
    }
    return { minX: minX - 40, minY: minY - 40, maxX: maxX + 40, maxY: maxY + 40 };
  });

  function fitToScreen() {
    if (!containerEl) return;
    const { minX, minY, maxX, maxY } = canvasBounds;
    const cw = maxX - minX;
    const ch = maxY - minY;
    const vw = containerEl.clientWidth;
    const vh = containerEl.clientHeight;
    if (cw === 0 || ch === 0) return;
    zoom = Math.min(vw / cw, vh / ch, 1.5) * 0.9;
    panX = (vw - cw * zoom) / 2 - minX * zoom;
    panY = (vh - ch * zoom) / 2 - minY * zoom;
  }

  function centerOnTable(tableName: string) {
    if (!containerEl) return;
    const pos = nodePositions[tableName];
    if (!pos) return;
    const vw = containerEl.clientWidth;
    const vh = containerEl.clientHeight;
    panX = vw / 2 - (pos.x + pos.width / 2) * zoom;
    panY = vh / 2 - (pos.y + pos.height / 2) * zoom;
  }

  // Pan/zoom handlers
  function handleWheel(e: WheelEvent) {
    e.preventDefault();
    const delta = e.deltaY > 0 ? 0.9 : 1.1;
    const newZoom = Math.min(Math.max(zoom * delta, 0.1), 3);
    // Zoom toward cursor position
    const rect = containerEl!.getBoundingClientRect();
    const mx = e.clientX - rect.left;
    const my = e.clientY - rect.top;
    panX = mx - (mx - panX) * (newZoom / zoom);
    panY = my - (my - panY) * (newZoom / zoom);
    zoom = newZoom;
  }

  function handleCanvasPointerDown(e: PointerEvent) {
    if (e.target === containerEl || (e.target as HTMLElement).classList.contains('erd-canvas-inner')) {
      isPanning = true;
      panStart = { x: e.clientX, y: e.clientY, panX, panY };
      containerEl!.setPointerCapture(e.pointerId);
      selectedTable = null;
    }
  }

  function handleCanvasPointerMove(e: PointerEvent) {
    if (isPanning) {
      panX = panStart.panX + (e.clientX - panStart.x);
      panY = panStart.panY + (e.clientY - panStart.y);
    } else if (draggingNode) {
      const pos = nodePositions[draggingNode];
      if (pos) {
        nodePositions[draggingNode] = {
          ...pos,
          x: (e.clientX - dragOffset.x - panX) / zoom,
          y: (e.clientY - dragOffset.y - panY) / zoom,
        };
        nodePositions = { ...nodePositions };
      }
    }
  }

  function handleCanvasPointerUp() {
    isPanning = false;
    draggingNode = null;
  }

  function handleNodePointerDown(tableName: string, e: PointerEvent) {
    e.stopPropagation();
    const pos = nodePositions[tableName];
    if (!pos) return;
    draggingNode = tableName;
    dragOffset = {
      x: e.clientX - pos.x * zoom - panX,
      y: e.clientY - pos.y * zoom - panY,
    };
    containerEl!.setPointerCapture(e.pointerId);
  }

  // Edge path calculation
  function getEdgePath(source: string, sourceCol: string, target: string, targetCol: string): string {
    const sp = nodePositions[source];
    const tp = nodePositions[target];
    if (!sp || !tp) return '';

    const sourceCols = columns[source] ?? [];
    const targetCols = columns[target] ?? [];
    const si = sourceCols.findIndex(c => c.name === sourceCol);
    const ti = targetCols.findIndex(c => c.name === targetCol);

    // Row height = ~19px, header = ~28px, padding = 2px
    const rowH = 19;
    const headerH = 30;
    const sy = sp.y + headerH + (si >= 0 ? si * rowH + rowH / 2 : 0);
    const ty = tp.y + headerH + (ti >= 0 ? ti * rowH + rowH / 2 : 0);

    // Determine side: connect from right of source to left of target, or vice versa
    let sx: number, tx: number;
    if (sp.x + sp.width < tp.x) {
      sx = sp.x + sp.width;
      tx = tp.x;
    } else if (tp.x + tp.width < sp.x) {
      sx = sp.x;
      tx = tp.x + tp.width;
    } else {
      sx = sp.x + sp.width;
      tx = tp.x + tp.width;
    }

    // Orthogonal path with rounded corners
    const midX = (sx + tx) / 2;
    const r = 6;

    if (Math.abs(sy - ty) < 2) {
      return `M ${sx} ${sy} L ${tx} ${ty}`;
    }

    return `M ${sx} ${sy} L ${midX - r} ${sy} Q ${midX} ${sy} ${midX} ${sy + (ty > sy ? r : -r)} L ${midX} ${ty + (ty > sy ? -r : r)} Q ${midX} ${ty} ${midX + r} ${ty} L ${tx} ${ty}`;
  }

  // Zoom controls (exposed via bind or events)
  export function zoomIn() {
    zoom = Math.min(zoom * 1.2, 3);
  }

  export function zoomOut() {
    zoom = Math.max(zoom * 0.8, 0.1);
  }

  export function fit() {
    fitToScreen();
  }

  export function relayout() {
    layoutDone = false;
    runLayout();
  }

  export function getZoom() {
    return zoom;
  }

  export function getCanvasElement() {
    return canvasEl;
  }

  // Search effect: center on first match
  $effect(() => {
    if (searchMatches && searchMatches.size > 0) {
      const first = searchMatches.values().next().value;
      if (first) centerOnTable(first);
    }
  });

  // Re-layout when the set of displayed tables changes
  let prevTableCount = $state(-1);
  $effect(() => {
    const count = displayTables.length;
    if (prevTableCount >= 0 && count !== prevTableCount) {
      layoutDone = false;
      runLayout();
    }
    prevTableCount = count;
  });

  onMount(() => {
    requestAnimationFrame(() => {
      runLayout();
    });
  });
</script>

<div
  bind:this={containerEl}
  class="relative flex-1 overflow-hidden bg-background cursor-grab"
  class:cursor-grabbing={isPanning}
  role="application"
  aria-label="ERD canvas"
  onwheel={handleWheel}
  onpointerdown={handleCanvasPointerDown}
  onpointermove={handleCanvasPointerMove}
  onpointerup={handleCanvasPointerUp}
>
  <div
    bind:this={canvasEl}
    class="erd-canvas-inner absolute origin-top-left"
    style:transform="translate({panX}px, {panY}px) scale({zoom})"
    style:will-change="transform"
  >
    <!-- SVG layer for edges (behind nodes) -->
    <svg
      class="absolute top-0 left-0 overflow-visible pointer-events-none"
      style:width="1px"
      style:height="1px"
    >
      {#each resolvedEdges as edge (edge.id)}
        {@const path = getEdgePath(edge.source, edge.sourceCol, edge.target, edge.targetCol)}
        {@const isHighlighted =
          highlightedFK &&
          ((edge.source === highlightedFK.table && edge.sourceCol === highlightedFK.column) ||
           (edge.target === highlightedFK.table && edge.targetCol === highlightedFK.column))
        }
        {@const isSelected =
          selectedTable && (edge.source === selectedTable || edge.target === selectedTable)
        }
        {#if path}
          <!-- Hover target (wider invisible path) -->
          <path
            d={path}
            fill="none"
            stroke="transparent"
            stroke-width="12"
            class="pointer-events-auto cursor-pointer"
          />
          <!-- Visible path -->
          <path
            d={path}
            fill="none"
            stroke="var(--primary)"
            stroke-width={isHighlighted ? 2 : 1.5}
            opacity={isHighlighted ? 1 : isSelected ? 0.8 : selectedTable ? 0.15 : 0.4}
            class="transition-opacity duration-150"
          />
          <!-- Dot at target end -->
          {@const tp = nodePositions[edge.target]}
          {@const targetCols = columns[edge.target] ?? []}
          {@const ti = targetCols.findIndex(c => c.name === edge.targetCol)}
          {@const ey = (tp?.y ?? 0) + 30 + (ti >= 0 ? ti * 19 + 9.5 : 0)}
          {@const sp = nodePositions[edge.source]}
          {@const ex = (sp?.x ?? 0) + (sp?.width ?? 0) < (tp?.x ?? 0) ? (tp?.x ?? 0) : (tp?.x ?? 0) + (tp?.width ?? 0)}
          <circle
            cx={ex}
            cy={ey}
            r="3"
            fill="var(--primary)"
            opacity={isHighlighted ? 1 : isSelected ? 0.8 : selectedTable ? 0.15 : 0.4}
          />
        {/if}
      {/each}
    </svg>

    <!-- Node layer -->
    {#each displayTables as table (table.name)}
      {@const pos = nodePositions[table.name]}
      {@const isSearchMatch = searchMatches ? searchMatches.has(table.name) : true}
      {@const highlightedCol = highlightedColumns.get(table.name)}
      {#if pos || !layoutDone}
        <ErdTableNode
          {table}
          columns={columns[table.name] ?? []}
          x={pos?.x ?? 0}
          y={pos?.y ?? 0}
          selected={selectedTable === table.name}
          dimmed={(selectedTable !== null && !connectedTables.has(table.name)) || (searchMatches !== null && !isSearchMatch)}
          highlightedColumn={highlightedCol?.values().next().value ?? null}
          onselect={() => { selectedTable = selectedTable === table.name ? null : table.name; }}
          ondblclick={() => onopentable?.(table.name)}
          onpointerdown={(e) => handleNodePointerDown(table.name, e)}
          oncolumnenter={(col) => { highlightedFK = { table: table.name, column: col }; }}
          oncolumnleave={() => { highlightedFK = null; }}
        />
      {/if}
    {/each}
  </div>

  <!-- Minimap -->
  <ErdMinimap
    nodes={Object.values(nodePositions)}
    viewportX={-panX / zoom}
    viewportY={-panY / zoom}
    viewportWidth={(containerEl?.clientWidth ?? 800) / zoom}
    viewportHeight={(containerEl?.clientHeight ?? 600) / zoom}
    canvasWidth={canvasBounds.maxX - canvasBounds.minX}
    canvasHeight={canvasBounds.maxY - canvasBounds.minY}
    onnavigate={(x, y) => { panX = -x * zoom; panY = -y * zoom; }}
  />

  <!-- Empty state -->
  {#if displayTables.length === 0}
    <div class="absolute inset-0 flex items-center justify-center">
      <p class="text-muted-foreground text-sm">No tables found in this schema</p>
    </div>
  {/if}
</div>
