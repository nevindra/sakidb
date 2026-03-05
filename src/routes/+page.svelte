<script lang="ts">
  import { onMount } from 'svelte';
  import { getAppState } from '$lib/stores';
  import { initRegistry, registerActions, setContexts, markComingSoon } from '$lib/commands';
  import type { CommandContext } from '$lib/commands';
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
  import SettingsDialog from '$lib/components/settings/SettingsDialog.svelte';
  import * as Tooltip from '$lib/components/ui/tooltip';

  const app = getAppState();
  const isMacOS = navigator.userAgent.includes('Macintosh');

  let commandPaletteOpen = $state(false);
  let updateDialogOpen = $state(false);
  let settingsOpen = $state(false);

  // Sidebar state
  const SIDEBAR_MIN = 180;
  const SIDEBAR_MAX = 480;
  const SIDEBAR_DEFAULT = 240;
  let sidebarWidth = $state(SIDEBAR_DEFAULT);
  let sidebarDragging = $state(false);
  let sidebarCollapsed = $state(false);
  let sidebarWidthBeforeCollapse = SIDEBAR_DEFAULT;

  function toggleSidebar() {
    if (sidebarCollapsed) {
      sidebarWidth = sidebarWidthBeforeCollapse;
      sidebarCollapsed = false;
    } else {
      sidebarWidthBeforeCollapse = sidebarWidth;
      sidebarWidth = 0;
      sidebarCollapsed = true;
    }
  }

  function onSidebarPointerDown(e: PointerEvent) {
    e.preventDefault();
    const target = e.currentTarget as HTMLElement;
    target.setPointerCapture(e.pointerId);
    sidebarDragging = true;
    document.body.style.userSelect = 'none';

    let rafId: number | null = null;

    function onPointerMove(e: PointerEvent) {
      if (rafId !== null) return;
      rafId = requestAnimationFrame(() => {
        rafId = null;
        sidebarWidth = Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, e.clientX));
        if (sidebarCollapsed) sidebarCollapsed = false;
      });
    }

    function onPointerUp() {
      sidebarDragging = false;
      document.body.style.userSelect = '';
      target.removeEventListener('pointermove', onPointerMove);
      target.removeEventListener('pointerup', onPointerUp);
      target.releasePointerCapture(e.pointerId);
      if (rafId !== null) cancelAnimationFrame(rafId);
    }

    target.addEventListener('pointermove', onPointerMove);
    target.addEventListener('pointerup', onPointerUp);
  }

  async function handleCheckForUpdates() {
    const found = await app.checkForUpdate();
    if (found) {
      updateDialogOpen = true;
    }
  }

  function openUpdateDialog() {
    updateDialogOpen = true;
  }

  // ── Helper to get first active connection + database ──
  function getFirstConnectionDb(): { savedConnectionId: string; databaseName: string } | null {
    const firstConn = [...app.activeConnections.values()][0];
    if (!firstConn) return null;
    const firstDb = [...firstConn.activeDatabases.keys()][0];
    if (!firstDb) return null;
    return { savedConnectionId: firstConn.savedConnectionId, databaseName: firstDb };
  }

  // ── Context tracking ──
  $effect(() => {
    const contexts: CommandContext[] = [];
    if (app.hasActiveConnections) contexts.push('connected');
    const tab = app.activeTab;
    if (tab) {
      if (tab.type === 'query') contexts.push('query-tab');
      else if (tab.type === 'data') contexts.push('data-tab');
      else if (tab.type === 'structure') contexts.push('structure-tab');
      else if (tab.type === 'erd') contexts.push('erd-tab');
    }
    setContexts(contexts);
  });

  // ── Register all command actions ──
  function registerAllActions() {
    const activeGroup = app.activeGroup;
    const activeGroupTabs = activeGroup?.tabIds ?? [];

    registerActions({
      // Navigation
      'nav.command-palette': () => { commandPaletteOpen = !commandPaletteOpen; },
      'nav.new-query': () => {
        const ctx = getFirstConnectionDb();
        if (ctx) app.openQueryTab(ctx.savedConnectionId, ctx.databaseName);
      },
      'nav.close-tab': () => {
        const tabId = app.activeTabId;
        if (tabId) app.closeTab(tabId);
      },
      'nav.next-tab': () => {
        const group = app.activeGroup;
        if (!group || group.tabIds.length < 2) return;
        const idx = group.activeTabId ? group.tabIds.indexOf(group.activeTabId) : -1;
        const next = group.tabIds[(idx + 1) % group.tabIds.length];
        app.setActiveTab(next, group.id);
      },
      'nav.prev-tab': () => {
        const group = app.activeGroup;
        if (!group || group.tabIds.length < 2) return;
        const idx = group.activeTabId ? group.tabIds.indexOf(group.activeTabId) : 0;
        const prev = group.tabIds[(idx - 1 + group.tabIds.length) % group.tabIds.length];
        app.setActiveTab(prev, group.id);
      },
      'nav.toggle-sidebar': toggleSidebar,
      'nav.focus-sidebar': () => {
        if (sidebarCollapsed) toggleSidebar();
        const el = document.querySelector('[data-sidebar-search]') as HTMLInputElement | null;
        el?.focus();
      },
      'nav.focus-editor': () => {
        const cm = document.querySelector('.cm-editor .cm-content') as HTMLElement | null;
        cm?.focus();
      },
      'nav.split-right': () => {
        const group = app.activeGroup;
        if (group) app.splitGroup(group.id, 'horizontal');
      },
      'nav.settings': () => { settingsOpen = !settingsOpen; },

      // Tab 1-9
      ...Object.fromEntries(
        Array.from({ length: 9 }, (_, i) => [
          `nav.tab-${i + 1}`,
          () => {
            const group = app.activeGroup;
            if (!group) return;
            const tabId = group.tabIds[i];
            if (tabId) app.setActiveTab(tabId, group.id);
          },
        ])
      ),

      // Query
      'query.execute': () => {
        const tab = app.activeTab;
        if (tab?.type === 'query') app.executeQueryInTab(tab.id, tab.content);
      },
      'query.cancel': () => {
        const tab = app.activeTab;
        if (tab?.type === 'query') app.cancelQuery(tab.runtimeConnectionId);
      },
      'query.save': () => {
        // TODO: open save query dialog
      },
      'query.switch-database': () => {
        // TODO: open database picker
      },
      'query.switch-schema': () => {
        // TODO: open schema picker
      },
      'query.set-timeout': () => {
        // TODO: open timeout dialog
      },

      // Saved queries
      'saved.list': () => {
        // TODO: focus saved queries in sidebar
      },
      'saved.delete': () => {
        // TODO: delete selected saved query
      },
      'saved.clear-history': () => { app.clearHistory(); },

      // Connection
      'conn.new': () => { app.openEditDialog(''); },
      'conn.disconnect': () => {
        const firstConn = [...app.activeConnections.values()][0];
        if (firstConn) app.disconnectFromDatabase(firstConn.savedConnectionId);
      },
      'conn.refresh': () => {
        const firstConn = [...app.activeConnections.values()][0];
        if (firstConn) app.refreshDatabases(firstConn.savedConnectionId);
      },

      // Database management
      'db.create': () => {
        // TODO: open create database dialog
      },
      'db.drop': () => {
        // TODO: open drop database dialog
      },
      'db.rename': () => {
        // TODO: open rename database dialog
      },

      // Data
      'data.refresh': () => {
        const tab = app.activeTab;
        if (tab?.type === 'data') app.loadDataTab(tab.id);
      },
      'data.clear-filters': () => {
        const tab = app.activeTab;
        if (tab?.type === 'data') app.updateDataTabFilters(tab.id, []);
      },
      'data.page-size': () => {
        // TODO: open page size picker
      },

      // Export / Import (dialog-driven — TODO: trigger export dialog for active tab)
      'export.csv': () => {
        // TODO: open export dialog in CSV mode for active data tab
      },
      'export.sql': () => {
        // TODO: open export dialog in SQL mode for active data tab
      },
      'import.restore-sql': () => {
        // TODO: open restore dialog
      },
      'import.cancel': () => {
        app.cancelRestore();
        // cancelExport requires connection context — TODO: cancel active export
      },

      // Structure
      'structure.copy-ddl': () => {
        const tab = app.activeTab;
        if (tab?.type === 'structure') {
          app.getCreateTableSql(tab.savedConnectionId, tab.databaseName, tab.schema, tab.table)
            .then(sql => navigator.clipboard.writeText(sql));
        }
      },
      'structure.profile': () => {
        const tab = app.activeTab;
        if (tab?.type === 'structure') app.loadProfilingData(tab.id);
      },

      // Layout
      'layout.reset': () => {
        const tabIds = app.tabs.map(t => t.id);
        // resetLayout is available from stores but not exposed via getAppState currently
        // Use the layout reset via the store
      },

      // App
      'app.check-update': handleCheckForUpdates,
      'app.clear-error': () => {
        if (commandPaletteOpen) {
          commandPaletteOpen = false;
        } else if (app.error) {
          app.clearError();
        }
      },
      'app.about': () => {
        // TODO: open about dialog
      },
    });
  }

  onMount(async () => {
    await app.init();
    registerAllActions();
    markComingSoon([
      'query.save',
      'query.switch-database',
      'query.switch-schema',
      'query.set-timeout',
      'saved.list',
      'saved.delete',
      'db.create',
      'db.drop',
      'db.rename',
      'data.page-size',
      'export.csv',
      'export.sql',
      'import.restore-sql',
      'layout.reset',
      'app.about',
    ]);
    await initRegistry();
  });
</script>

{#if app.hasActiveConnections}
  <!-- Connected workspace -->
  <Tooltip.Provider delayDuration={300}>
    <div class="flex flex-col h-screen bg-background text-foreground">
      {#if !isMacOS}<TitleBar onCommandPalette={() => (commandPaletteOpen = true)} onCheckForUpdates={handleCheckForUpdates} />{/if}
      <UpdateBanner onUpdate={openUpdateDialog} />
      <div class="flex flex-1 overflow-hidden">
        {#if !sidebarCollapsed}
          <div class="shrink-0 overflow-hidden transition-[width] duration-150" style:width="{sidebarWidth}px">
            <Sidebar />
          </div>

          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="shrink-0 relative z-10 w-1 cursor-col-resize group"
            class:bg-primary={sidebarDragging}
            class:bg-border={!sidebarDragging}
            onpointerdown={onSidebarPointerDown}
          >
            <div class="absolute -left-1 -right-1 top-0 bottom-0"></div>
            {#if !sidebarDragging}
              <div class="absolute opacity-0 group-hover:opacity-100 transition-opacity duration-150 bg-primary inset-y-0 -left-px -right-px"></div>
            {/if}
          </div>
        {/if}

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

<CommandPalette bind:open={commandPaletteOpen} />
<UpdateDialog bind:open={updateDialogOpen} />
<SettingsDialog bind:open={settingsOpen} />
