<script lang="ts">
  import { getAppState } from '$lib/stores';
  import { X, Download } from '@lucide/svelte';
  import { Button } from '$lib/components/ui/button';

  let { onUpdate }: { onUpdate: () => void } = $props();

  const app = getAppState();

  const showBanner = $derived(app.update && !app.updateDismissed && !app.updateDownloading && !app.updateReadyToInstall);
</script>

{#if showBanner}
  <div class="flex items-center gap-2 px-3 h-8 bg-default/8 border-b border-info/15 text-xs shrink-0">
    <Download class="h-3 w-3 text-info" />
    <span class="text-muted-foreground">
      SakiDB <span class="text-foreground font-medium">v{app.update?.version}</span> is available
    </span>
    <Button variant="outline" size="sm" class="h-5 px-2 text-[11px] ml-1" onclick={onUpdate}>
      Update Now
    </Button>
    <button
      class="ml-auto text-muted-foreground hover:text-foreground transition-colors"
      onclick={() => app.dismissUpdateBanner()}
      aria-label="Dismiss"
    >
      <X class="h-3 w-3" />
    </button>
  </div>
{/if}
