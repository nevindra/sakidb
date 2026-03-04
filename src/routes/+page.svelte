<script lang="ts">
  import { onMount } from 'svelte';
  import { getAppState } from '$lib/stores';
  import ConnectionOnboarding from '$lib/components/sidebar/ConnectionOnboarding.svelte';
  import ConnectionManager from '$lib/components/sidebar/ConnectionManager.svelte';
  import ConnectionEditDialog from '$lib/components/sidebar/ConnectionEditDialog.svelte';
  import Sidebar from '$lib/components/sidebar/Sidebar.svelte';
  import SplitPane from '$lib/components/shell/SplitPane.svelte';
  import TitleBar from '$lib/components/shell/TitleBar.svelte';
  import Toast from '$lib/components/shell/Toast.svelte';
  import CommandPalette from '$lib/components/shell/CommandPalette.svelte';
  import UpdateBanner from '$lib/components/shell/UpdateBanner.svelte';
  import UpdateDialog from '$lib/components/shell/UpdateDialog.svelte';
  import * as Tooltip from '$lib/components/ui/tooltip';

  const app = getAppState();
  const isMacOS = navigator.userAgent.includes('Macintosh');

  let commandPaletteOpen = $state(false);
  let updateDialogOpen = $state(false);

  async function handleCheckForUpdates() {
    const found = await app.checkForUpdate();
    if (found) {
      updateDialogOpen = true;
    }
  }

  function openUpdateDialog() {
    updateDialogOpen = true;
  }

  const commands = $derived.by(() => {
    const cmds: { id: string; label: string; shortcut?: string; action: () => void }[] = [];

    // New query — only when connected
    const firstConn = [...app.activeConnections.values()][0];
    if (firstConn) {
      const firstDb = [...firstConn.activeDatabases.keys()][0];
      if (firstDb) {
        cmds.push({
          id: 'new-query',
          label: 'New Query',
          shortcut: 'Ctrl+N',
          action: () => app.openQueryTab(firstConn.savedConnectionId, firstDb),
        });
      }
    }

    cmds.push(
      { id: 'about', label: 'About SakiDB', action: () => {} },
    );

    return cmds;
  });

  onMount(() => {
    app.init();

    function onKeydown(e: KeyboardEvent) {
      if (e.key === 'k' && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        commandPaletteOpen = !commandPaletteOpen;
      }
    }

    window.addEventListener('keydown', onKeydown);
    return () => window.removeEventListener('keydown', onKeydown);
  });
</script>

{#if app.hasActiveConnections}
  <!-- Connected workspace -->
  <Tooltip.Provider delayDuration={300}>
    <div class="flex flex-col h-screen bg-background text-foreground">
      {#if !isMacOS}<TitleBar onCommandPalette={() => (commandPaletteOpen = true)} onCheckForUpdates={handleCheckForUpdates} />{/if}
      <UpdateBanner onUpdate={openUpdateDialog} />
      <div class="flex flex-1 overflow-hidden">
        <div class="w-60 shrink-0">
          <Sidebar />
        </div>

        <div class="flex flex-col flex-1 overflow-hidden min-w-0">
          <SplitPane node={app.layoutRoot} />
        </div>
      </div>
    </div>
  </Tooltip.Provider>

  <ConnectionEditDialog />
{:else if app.savedConnections.length === 0}
  <!-- No saved connections: onboarding -->
  <Tooltip.Provider delayDuration={300}>
    <div class="flex flex-col h-screen bg-background text-foreground">
      {#if !isMacOS}<TitleBar onCommandPalette={() => (commandPaletteOpen = true)} onCheckForUpdates={handleCheckForUpdates} />{/if}
      <UpdateBanner onUpdate={openUpdateDialog} />
      <div class="flex-1 overflow-hidden">
        <ConnectionOnboarding />
      </div>
    </div>
  </Tooltip.Provider>
{:else}
  <!-- Has saved connections but not connected -->
  <Tooltip.Provider delayDuration={300}>
    <div class="flex flex-col h-screen bg-background text-foreground">
      {#if !isMacOS}<TitleBar onCommandPalette={() => (commandPaletteOpen = true)} onCheckForUpdates={handleCheckForUpdates} />{/if}
      <UpdateBanner onUpdate={openUpdateDialog} />
      <div class="flex-1 overflow-hidden">
        <ConnectionManager />
      </div>
    </div>
  </Tooltip.Provider>
{/if}

{#if app.error}
  <Toast message={app.error} onDismiss={() => app.clearError()} />
{/if}

<CommandPalette {commands} bind:open={commandPaletteOpen} />
<UpdateDialog bind:open={updateDialogOpen} />
