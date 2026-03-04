<script lang="ts">
  interface Props {
    direction: 'horizontal' | 'vertical';
    onResize: (ratio: number) => void;
    onResizeStart?: () => void;
    onResizeEnd?: () => void;
  }

  let { direction, onResize, onResizeStart, onResizeEnd }: Props = $props();

  let dragging = $state(false);

  function handlePointerDown(e: PointerEvent) {
    e.preventDefault();
    const target = e.currentTarget as HTMLElement;
    target.setPointerCapture(e.pointerId);
    dragging = true;
    document.body.style.userSelect = 'none';
    onResizeStart?.();

    const container = target.parentElement;
    if (!container) return;

    let rafId: number | null = null;

    function onPointerMove(e: PointerEvent) {
      if (rafId !== null) return;
      rafId = requestAnimationFrame(() => {
        rafId = null;
        const rect = container!.getBoundingClientRect();
        let ratio: number;
        if (direction === 'horizontal') {
          ratio = (e.clientX - rect.left) / rect.width;
        } else {
          ratio = (e.clientY - rect.top) / rect.height;
        }
        onResize(ratio);
      });
    }

    function onPointerUp() {
      dragging = false;
      document.body.style.userSelect = '';
      target.removeEventListener('pointermove', onPointerMove);
      target.removeEventListener('pointerup', onPointerUp);
      target.releasePointerCapture(e.pointerId);
      if (rafId !== null) {
        cancelAnimationFrame(rafId);
      }
      onResizeEnd?.();
    }

    target.addEventListener('pointermove', onPointerMove);
    target.addEventListener('pointerup', onPointerUp);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="shrink-0 relative z-10 group {direction === 'horizontal' ? 'w-1 cursor-col-resize' : 'h-1 cursor-row-resize'}"
  class:bg-primary={dragging}
  class:bg-border={!dragging}
  onpointerdown={handlePointerDown}
>
  <!-- Wider hit area -->
  <div
    class="absolute {direction === 'horizontal' ? '-left-1 -right-1 top-0 bottom-0' : 'left-0 right-0 -top-1 -bottom-1'}"
  ></div>
  <!-- Hover highlight -->
  {#if !dragging}
    <div
      class="absolute opacity-0 group-hover:opacity-100 transition-opacity duration-150 bg-primary {direction === 'horizontal' ? 'inset-y-0 -left-px -right-px' : 'inset-x-0 -top-px -bottom-px'}"
    ></div>
  {/if}
</div>
