<script lang="ts">
  import { getAppState } from '$lib/stores';
  import * as Dialog from '$lib/components/ui/dialog';
  import { Button } from '$lib/components/ui/button';
  import { Download, RotateCw } from '@lucide/svelte';

  let { open = $bindable(false) }: { open?: boolean } = $props();

  const app = getAppState();

  const progress = $derived(
    app.updateContentLength > 0
      ? Math.round((app.updateDownloaded / app.updateContentLength) * 100)
      : 0
  );

  const downloadedMB = $derived((app.updateDownloaded / 1024 / 1024).toFixed(1));
  const totalMB = $derived((app.updateContentLength / 1024 / 1024).toFixed(1));

  async function startUpdate() {
    await app.downloadAndInstall();
  }

  function handleOpenChange(value: boolean) {
    // Prevent closing while downloading
    if (app.updateDownloading) return;
    open = value;
  }
</script>

<Dialog.Root bind:open onOpenChange={handleOpenChange}>
  <Dialog.Content showCloseButton={!app.updateDownloading} class="max-w-md">
    <Dialog.Header>
      <Dialog.Title>
        {#if app.updateReadyToInstall}
          Ready to restart
        {:else if app.updateDownloading}
          Downloading update...
        {:else}
          Update available
        {/if}
      </Dialog.Title>
      <Dialog.Description>
        {#if app.update}
          SakiDB v{app.update.version}
        {/if}
      </Dialog.Description>
    </Dialog.Header>

    <div class="py-2">
      {#if app.updateReadyToInstall}
        <p class="text-sm text-muted-foreground">
          Update has been downloaded. Restart SakiDB to apply the update.
        </p>
      {:else if app.updateDownloading}
        <!-- Progress bar -->
        <div class="space-y-2">
          <div class="h-1.5 w-full rounded-full bg-accent overflow-hidden">
            <div
              class="h-full rounded-full bg-primary transition-all duration-300 ease-out"
              style="width: {progress}%"
            ></div>
          </div>
          <p class="text-xs text-muted-foreground text-right">
            {#if app.updateContentLength > 0}
              {downloadedMB} / {totalMB} MB ({progress}%)
            {:else}
              Downloading...
            {/if}
          </p>
        </div>
      {:else}
        {#if app.update?.body}
          <div class="max-h-48 overflow-y-auto rounded-md bg-background border border-border/60 p-3 text-xs text-muted-foreground leading-relaxed whitespace-pre-wrap">
            {app.update.body}
          </div>
        {:else}
          <p class="text-sm text-muted-foreground">
            A new version is ready to download and install.
          </p>
        {/if}
      {/if}
    </div>

    <Dialog.Footer>
      {#if app.updateReadyToInstall}
        <Button variant="outline" size="sm" onclick={() => (open = false)}>
          Later
        </Button>
        <Button variant="default" size="sm" onclick={() => app.restartApp()}>
          <RotateCw class="h-3 w-3 mr-1.5" />
          Restart Now
        </Button>
      {:else if app.updateDownloading}
        <p class="text-xs text-muted-foreground">Please wait...</p>
      {:else}
        <Button variant="outline" size="sm" onclick={() => (open = false)}>
          Cancel
        </Button>
        <Button variant="default" size="sm" onclick={startUpdate}>
          <Download class="h-3 w-3 mr-1.5" />
          Download & Install
        </Button>
      {/if}
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
