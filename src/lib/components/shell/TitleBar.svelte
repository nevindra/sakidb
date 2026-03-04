<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { getAppState } from '$lib/stores';
  import { Menu, Minus, Square, X } from '@lucide/svelte';
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu';

  let { onCommandPalette, onCheckForUpdates, isMacOS = false }: { onCommandPalette?: () => void; onCheckForUpdates?: () => void; isMacOS?: boolean } = $props();

  const app = getAppState();
  const appWindow = getCurrentWindow();

  function newQueryTab() {
    const firstConn = [...app.activeConnections.values()][0];
    if (!firstConn) return;
    const firstDb = [...firstConn.activeDatabases.keys()][0];
    if (firstDb) {
      app.openQueryTab(firstConn.savedConnectionId, firstDb);
    }
  }

  const canNewQuery = $derived(app.activeConnections.size > 0);

  let isMaximized = $state(false);

  $effect(() => {
    appWindow.isMaximized().then((v) => (isMaximized = v));

    const unlisten = appWindow.onResized(async () => {
      isMaximized = await appWindow.isMaximized();
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  });
</script>

<div class="flex items-center h-8 bg-background shrink-0 select-none" class:pl-[72px]={isMacOS}>
  <!-- Hamburger menu -->
  <DropdownMenu.Root>
    <DropdownMenu.Trigger
      class="h-8 w-10 flex items-center justify-center text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors duration-100"
    >
      <Menu class="h-3.5 w-3.5" />
    </DropdownMenu.Trigger>
    <DropdownMenu.Content align="start" sideOffset={2} class="min-w-[200px]">
      {#if canNewQuery}
        <DropdownMenu.Item onclick={newQueryTab}>
          New Query
          <DropdownMenu.Shortcut>Ctrl+N</DropdownMenu.Shortcut>
        </DropdownMenu.Item>
        <DropdownMenu.Separator />
      {/if}
      <DropdownMenu.Item onclick={() => onCommandPalette?.()}>
        Command Palette
        <DropdownMenu.Shortcut>Ctrl+K</DropdownMenu.Shortcut>
      </DropdownMenu.Item>
      <DropdownMenu.Separator />
      <DropdownMenu.Item>
        Settings
        <DropdownMenu.Shortcut>Ctrl+,</DropdownMenu.Shortcut>
      </DropdownMenu.Item>
      <DropdownMenu.Item onclick={() => onCheckForUpdates?.()}>Check for Updates</DropdownMenu.Item>
      <DropdownMenu.Separator />
      <DropdownMenu.Item>About SakiDB</DropdownMenu.Item>
    </DropdownMenu.Content>
  </DropdownMenu.Root>

  <!-- Title / drag region -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="flex-1 flex items-center px-2 h-full text-xs text-muted-foreground"
    data-tauri-drag-region
    ondblclick={() => appWindow.toggleMaximize()}
  >
    <span class="text-[11px] font-semibold tracking-wider uppercase" data-tauri-drag-region>
      SakiDB
    </span>
  </div>

  <!-- Window controls -->
  <button
    class="h-8 w-10 flex items-center justify-center text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors duration-100"
    onclick={() => appWindow.minimize()}
    aria-label="Minimize"
  >
    <Minus class="h-3.5 w-3.5" />
  </button>
  <button
    class="h-8 w-10 flex items-center justify-center text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors duration-100"
    onclick={() => appWindow.toggleMaximize()}
    aria-label={isMaximized ? 'Restore' : 'Maximize'}
  >
    {#if isMaximized}
      <svg class="h-3 w-3" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.2">
        <rect x="2" y="3" width="6" height="6" rx="0.5" />
        <path d="M3.5 3V2.5A.5.5 0 0 1 4 2h4.5a.5.5 0 0 1 .5.5V7a.5.5 0 0 1-.5.5H8" />
      </svg>
    {:else}
      <Square class="h-3 w-3" />
    {/if}
  </button>
  <button
    class="h-8 w-10 flex items-center justify-center text-muted-foreground hover:text-foreground hover:bg-destructive/80 hover:text-white transition-colors duration-100"
    onclick={() => appWindow.close()}
    aria-label="Close"
  >
    <X class="h-3.5 w-3.5" />
  </button>
</div>
