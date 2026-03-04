<script lang="ts">
  let {
    nodes,
    viewportX,
    viewportY,
    viewportWidth,
    viewportHeight,
    canvasWidth,
    canvasHeight,
    onnavigate,
  }: {
    nodes: { x: number; y: number; width: number; height: number }[];
    viewportX: number;
    viewportY: number;
    viewportWidth: number;
    viewportHeight: number;
    canvasWidth: number;
    canvasHeight: number;
    onnavigate?: (x: number, y: number) => void;
  } = $props();

  const MINIMAP_W = 150;
  const MINIMAP_H = 100;

  const scale = $derived(
    canvasWidth > 0 && canvasHeight > 0
      ? Math.min(MINIMAP_W / canvasWidth, MINIMAP_H / canvasHeight)
      : 1
  );

  let dragging = $state(false);

  function handlePointerDown(e: PointerEvent) {
    dragging = true;
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
    navigateTo(e);
  }

  function handlePointerMove(e: PointerEvent) {
    if (!dragging) return;
    navigateTo(e);
  }

  function handlePointerUp() {
    dragging = false;
  }

  function navigateTo(e: PointerEvent) {
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const mx = (e.clientX - rect.left) / scale;
    const my = (e.clientY - rect.top) / scale;
    onnavigate?.(mx - viewportWidth / 2, my - viewportHeight / 2);
  }
</script>

{#if nodes.length > 0}
  <div
    class="absolute bottom-3 right-3 bg-card/90 border border-border rounded-md overflow-hidden backdrop-blur-sm"
    style:width="{MINIMAP_W}px"
    style:height="{MINIMAP_H}px"
    role="navigation"
    aria-label="Diagram minimap"
    onpointerdown={handlePointerDown}
    onpointermove={handlePointerMove}
    onpointerup={handlePointerUp}
  >
    <svg width={MINIMAP_W} height={MINIMAP_H}>
      <!-- Table nodes -->
      {#each nodes as node}
        <rect
          x={node.x * scale}
          y={node.y * scale}
          width={Math.max(node.width * scale, 2)}
          height={Math.max(node.height * scale, 2)}
          fill="var(--secondary)"
          stroke="var(--border)"
          stroke-width="0.5"
          rx="1"
        />
      {/each}

      <!-- Viewport indicator -->
      <rect
        x={viewportX * scale}
        y={viewportY * scale}
        width={viewportWidth * scale}
        height={viewportHeight * scale}
        fill="none"
        stroke="var(--primary)"
        stroke-width="1"
        rx="1"
        opacity="0.6"
      />
    </svg>
  </div>
{/if}
