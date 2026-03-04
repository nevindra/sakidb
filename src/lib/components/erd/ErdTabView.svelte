<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { ErdTab } from '$lib/types';
  import ErdToolbar from './ErdToolbar.svelte';
  import ErdCanvas from './ErdCanvas.svelte';
  import { Loader2 } from '@lucide/svelte';

  let { tab }: { tab: ErdTab } = $props();

  const app = getAppState();
  let canvas: ErdCanvas | undefined = $state();
  let searchQuery = $state('');
  let currentZoom = $state(1);
  let simplified = $state(true);

  // Track zoom from canvas
  $effect(() => {
    if (canvas) {
      const interval = setInterval(() => {
        currentZoom = canvas!.getZoom();
      }, 100);
      return () => clearInterval(interval);
    }
  });

  function downloadDataUrl(dataUrl: string, filename: string) {
    const link = document.createElement('a');
    link.download = filename;
    link.href = dataUrl;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
  }

  async function handleExportPng() {
    const el = canvas?.getCanvasElement();
    if (!el) return;
    try {
      const { toPng } = await import('html-to-image');
      const dataUrl = await toPng(el, {
        backgroundColor: '#131316',
        pixelRatio: 2,
      });
      downloadDataUrl(dataUrl, `erd-${tab.schema}.png`);
    } catch (e) {
      console.error('Export PNG failed:', e);
    }
  }

  async function handleExportSvg() {
    const el = canvas?.getCanvasElement();
    if (!el) return;
    try {
      const { toSvg } = await import('html-to-image');
      const dataUrl = await toSvg(el, {
        backgroundColor: '#131316',
      });
      downloadDataUrl(dataUrl, `erd-${tab.schema}.svg`);
    } catch (e) {
      console.error('Export SVG failed:', e);
    }
  }

  function handleOpenTable(tableName: string) {
    app.openStructureTab(tab.savedConnectionId, tab.databaseName, tab.schema, tableName);
  }
</script>

<div class="flex flex-col flex-1 overflow-hidden">
  {#if tab.isLoading}
    <div class="flex-1 flex items-center justify-center">
      <Loader2 class="h-5 w-5 animate-spin text-muted-foreground" />
      <span class="ml-2 text-sm text-muted-foreground">Loading schema...</span>
    </div>
  {:else}
    <ErdToolbar
      zoom={currentZoom}
      {searchQuery}
      {simplified}
      onzoomin={() => canvas?.zoomIn()}
      onzoomout={() => canvas?.zoomOut()}
      onfit={() => canvas?.fit()}
      onrelayout={() => canvas?.relayout()}
      onexportpng={handleExportPng}
      onexportsvg={handleExportSvg}
      onsearch={(q) => { searchQuery = q; }}
      ontogglemode={() => { simplified = !simplified; }}
    />
    <ErdCanvas
      bind:this={canvas}
      tables={tab.tables}
      columns={tab.columns}
      foreignKeys={tab.foreignKeys}
      focusTable={tab.focusTable}
      {searchQuery}
      {simplified}
      onopentable={handleOpenTable}
    />
  {/if}
</div>
