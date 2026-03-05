<script lang="ts">
  import * as Dialog from '$lib/components/ui/dialog';
  import { getVersion } from '@tauri-apps/api/app';
  import { onMount } from 'svelte';

  let { open = $bindable(false) }: { open: boolean } = $props();

  let appVersion = $state('');

  onMount(async () => {
    appVersion = await getVersion();
  });
</script>

<Dialog.Root bind:open>
  <Dialog.Content showCloseButton={false} class="max-w-xs">
    <div class="flex flex-col items-center gap-4 py-4">
      <div class="flex flex-col items-center gap-1">
        <h2 class="text-lg font-semibold">SakiDB</h2>
        <span class="text-xs text-muted-foreground">Version {appVersion}</span>
      </div>

      <p class="text-xs text-muted-foreground text-center leading-relaxed">
        A lightweight, fast PostgreSQL client built with Tauri and Svelte.
      </p>

      <div class="text-[10px] text-muted-foreground/50">
        Tauri v2 + Svelte 5
      </div>
    </div>
  </Dialog.Content>
</Dialog.Root>
