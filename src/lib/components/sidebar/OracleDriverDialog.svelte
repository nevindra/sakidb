<script lang="ts">
  import * as Dialog from '$lib/components/ui/dialog';
  import { getAppState } from '$lib/stores';
  import { Loader2, Download, Info, ExternalLink } from '@lucide/svelte';
  import { onMount } from 'svelte';

  let { onDownloadComplete } = $props<{ 
    onDownloadComplete?: () => void;
  }>();

  const app = getAppState();

  onMount(() => {
    console.log('OracleDriverDialog mounted');
  });

  const progress = $derived(app.oracleDownloadProgress);
  const isDownloading = $derived(app.isOracleDownloading);

  async function handleDownload() {
    await app.downloadOracleDriver();
    if (app.oracleDriverStatus?.found) {
      app.isOracleDriverDialogOpen = false;
      onDownloadComplete?.();
    }
  }
</script>

<Dialog.Root bind:open={app.isOracleDriverDialogOpen}>
  <Dialog.Content class="sm:max-w-[480px]">
    <Dialog.Header>
      <Dialog.Title>Oracle Instant Client Required</Dialog.Title>
      <Dialog.Description>
        To connect to Oracle databases, the Oracle Instant Client libraries must be installed on your system.
      </Dialog.Description>
    </Dialog.Header>

    <div class="space-y-4 py-4">
      <div class="flex items-start gap-3 p-3 rounded-lg bg-accent/10 border border-border/10">
        <Info class="h-4 w-4 text-blue-400 mt-0.5 shrink-0" />
        <div class="text-[13px] space-y-2">
          <p>SakiDB uses ODPI-C which requires these libraries. You can download them automatically or install them manually following the official guide.</p>
          <a 
            href="https://odpi-c.readthedocs.io/en/latest/user_guide/installation.html" 
            target="_blank"
            class="flex items-center gap-1 text-blue-400 hover:underline font-medium"
          >
            Official ODPI-C Installation Guide <ExternalLink class="h-3 w-3" />
          </a>
        </div>
      </div>

      {#if isDownloading && progress}
        <div class="space-y-2.5 py-2">
          <div class="flex justify-between text-[12px] font-medium">
            <span class="text-foreground/70">{progress.message}</span>
            <span class="text-foreground">{Math.round(progress.progress)}%</span>
          </div>
          <div class="h-1.5 w-full bg-accent/20 rounded-full overflow-hidden">
            <div 
              class="h-full bg-primary transition-all duration-300 ease-out" 
              style="width: {progress.progress}%"
            ></div>
          </div>
        </div>
      {:else}
        <div class="space-y-3">
          <h4 class="text-[12px] font-semibold text-foreground/80 uppercase tracking-wider">Manual Installation Paths</h4>
          <ul class="text-[12px] text-text-dim/80 space-y-1.5 list-disc pl-4">
            <li>MacOS: <code class="bg-accent/20 px-1 rounded">~/lib/libclntsh.dylib</code> or <code class="bg-accent/20 px-1 rounded">/usr/local/lib/</code></li>
            <li>MacOS (ARM): <code class="bg-accent/20 px-1 rounded">/opt/homebrew/lib/</code></li>
            <li>Linux: <code class="bg-accent/20 px-1 rounded">/usr/lib</code> or via <code class="bg-accent/20 px-1 rounded">ldconfig</code></li>
            <li>Windows: Any folder in your system <code class="bg-accent/20 px-1 rounded">PATH</code></li>
          </ul>
        </div>
      {/if}
    </div>

    <Dialog.Footer class="gap-2">
      <button
        class="h-9 px-4 text-[13px] font-medium rounded-md border border-border/20 hover:bg-accent/10 transition-colors disabled:opacity-50"
        onclick={() => app.isOracleDriverDialogOpen = false}
        disabled={isDownloading}
      >
        Cancel
      </button>
      <button
        class="h-9 px-4 text-[13px] font-medium rounded-md bg-primary text-primary-foreground hover:brightness-110 active:brightness-95 transition-all flex items-center gap-2 disabled:opacity-70"
        onclick={handleDownload}
        disabled={isDownloading}
      >
        {#if isDownloading}
          <Loader2 class="h-3.5 w-3.5 animate-spin" />
          Downloading...
        {:else}
          <Download class="h-3.5 w-3.5" />
          Download Automatically
        {/if}
      </button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
