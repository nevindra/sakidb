<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { getAppState } from '$lib/stores';
  import { resetLayout } from '$lib/stores/layout.svelte';
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
  import AboutDialog from '$lib/components/shell/AboutDialog.svelte';
  import ListPickerDialog from '$lib/components/shell/ListPickerDialog.svelte';
  import type { ListPickerItem } from '$lib/components/shell/ListPickerDialog.svelte';
  import InputDialog from '$lib/components/ui/input-dialog/InputDialog.svelte';
  import ConfirmDialog from '$lib/components/ui/confirm-dialog/ConfirmDialog.svelte';
  import ExportDialog from '$lib/components/structure/ExportDialog.svelte';
  import RestoreDialog from '$lib/components/sidebar/tree/RestoreDialog.svelte';
  import * as Tooltip from '$lib/components/ui/tooltip';

  const app = getAppState();
  const isMacOS = navigator.userAgent.includes('Macintosh');

  let commandPaletteOpen = $state(false);
  let updateDialogOpen = $state(false);
  let settingsOpen = $state(false);
  let aboutOpen = $state(false);

  // Dialog state for command actions
  let saveQueryOpen = $state(false);
  let saveQuerySql = $state('');
  let saveQueryConnectionId = $state<string | null>(null);
  let saveQueryDatabaseName = $state<string | null>(null);

  let dbCreateOpen = $state(false);
  let dbCreateConnectionId = $state('');

  let dbDropOpen = $state(false);
  let dbDropConnectionId = $state('');
  let dbDropDatabaseName = $state('');

  let dbRenameOpen = $state(false);
  let dbRenameConnectionId = $state('');
  let dbRenameOldName = $state('');

  let timeoutOpen = $state(false);
  let timeoutTabId = $state('');
  let timeoutInitialValue = $state('');

  let pickerOpen = $state(false);
  let pickerTitle = $state('');
  let pickerItems = $state<ListPickerItem[]>([]);
  let pickerOnSelect = $state<(value: string) => void>(() => {});

  let exportOpen = $state(false);
  let exportConnectionId = $state('');
  let exportDatabaseName = $state('');
  let exportSchema = $state('');
  let exportTable = $state('');
  let exportWhereClause = $state<string | undefined>(undefined);

  let restoreOpen = $state(false);
  let restoreConnectionId = $state('');
  let restoreDatabaseName = $state('');

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
        const tab = app.activeTab;
        if (tab?.type !== 'query' || !tab.content.trim()) return;
        saveQuerySql = tab.content;
        saveQueryConnectionId = tab.savedConnectionId;
        saveQueryDatabaseName = tab.databaseName;
        saveQueryOpen = true;
      },
      'query.switch-database': () => {
        const tab = app.activeTab;
        if (tab?.type !== 'query') return;
        const dbs = app.getDatabases(tab.savedConnectionId);
        pickerTitle = 'Switch Database';
        pickerItems = dbs.map(db => ({
          value: db.name,
          label: db.name,
          description: db.name === tab.databaseName ? 'current' : undefined,
        }));
        pickerOnSelect = (value: string) => app.switchQueryTabDatabase(tab.id, value);
        pickerOpen = true;
      },
      'query.switch-schema': () => {
        const tab = app.activeTab;
        if (tab?.type !== 'query') return;
        const schemas = app.getSchemas(tab.savedConnectionId, tab.databaseName);
        pickerTitle = 'Switch Schema';
        pickerItems = schemas.map(s => ({
          value: s.name,
          label: s.name,
          description: s.name === tab.schemaName ? 'current' : undefined,
        }));
        pickerOnSelect = (value: string) => app.switchQueryTabSchema(tab.id, value);
        pickerOpen = true;
      },
      'query.set-timeout': () => {
        const tab = app.activeTab;
        if (tab?.type !== 'query') return;
        timeoutTabId = tab.id;
        timeoutInitialValue = tab.statementTimeoutMs ? String(tab.statementTimeoutMs / 1000) : '';
        timeoutOpen = true;
      },

      // Saved queries
      'saved.list': () => {
        if (sidebarCollapsed) toggleSidebar();
        const btn = document.querySelector('[data-sidebar-queries-tab]') as HTMLButtonElement | null;
        btn?.click();
      },
      'saved.delete': () => {
        // Requires selection context from sidebar — not actionable from palette
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
        const ctx = getFirstConnectionDb();
        if (!ctx) return;
        dbCreateConnectionId = ctx.savedConnectionId;
        dbCreateOpen = true;
      },
      'db.drop': () => {
        const ctx = getFirstConnectionDb();
        if (!ctx) return;
        const dbs = app.getDatabases(ctx.savedConnectionId);
        pickerTitle = 'Drop Database';
        pickerItems = dbs.map(db => ({ value: db.name, label: db.name }));
        pickerOnSelect = (value: string) => {
          dbDropConnectionId = ctx.savedConnectionId;
          dbDropDatabaseName = value;
          dbDropOpen = true;
        };
        pickerOpen = true;
      },
      'db.rename': () => {
        const ctx = getFirstConnectionDb();
        if (!ctx) return;
        const dbs = app.getDatabases(ctx.savedConnectionId);
        pickerTitle = 'Rename Database';
        pickerItems = dbs.map(db => ({ value: db.name, label: db.name }));
        pickerOnSelect = (value: string) => {
          dbRenameConnectionId = ctx.savedConnectionId;
          dbRenameOldName = value;
          dbRenameOpen = true;
        };
        pickerOpen = true;
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
        const tab = app.activeTab;
        if (tab?.type !== 'data') return;
        const sizes = [25, 50, 100, 250, 500, 1000];
        pickerTitle = 'Set Page Size';
        pickerItems = sizes.map(s => ({
          value: String(s),
          label: `${s} rows`,
          description: s === tab.pageSize ? 'current' : undefined,
        }));
        pickerOnSelect = (value: string) => app.updateDataTabPageSize(tab.id, Number(value));
        pickerOpen = true;
      },

      // Export / Import
      'export.csv': () => {
        const tab = app.activeTab;
        if (tab?.type !== 'data') return;
        exportConnectionId = tab.savedConnectionId;
        exportDatabaseName = tab.databaseName;
        exportSchema = tab.schema;
        exportTable = tab.table;
        exportWhereClause = undefined;
        exportOpen = true;
      },
      'export.sql': () => {
        const tab = app.activeTab;
        if (tab?.type !== 'data') return;
        exportConnectionId = tab.savedConnectionId;
        exportDatabaseName = tab.databaseName;
        exportSchema = tab.schema;
        exportTable = tab.table;
        exportWhereClause = undefined;
        exportOpen = true;
      },
      'import.restore-sql': () => {
        const ctx = getFirstConnectionDb();
        if (!ctx) return;
        restoreConnectionId = ctx.savedConnectionId;
        restoreDatabaseName = ctx.databaseName;
        restoreOpen = true;
      },
      'import.cancel': () => {
        app.cancelRestore();
        const ctx = getFirstConnectionDb();
        if (ctx) app.cancelExport(ctx.savedConnectionId, ctx.databaseName);
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
        resetLayout(tabIds);
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
      'app.about': () => { aboutOpen = true; },
    });
  }

  onMount(async () => {
    await app.init();
    registerAllActions();
    markComingSoon([
      'saved.delete',
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
<AboutDialog bind:open={aboutOpen} />

<!-- Save query dialog -->
<InputDialog
  bind:open={saveQueryOpen}
  title="Save Query"
  description="Give this query a name to save it."
  label="Name"
  placeholder="e.g. Monthly report"
  confirmLabel="Save"
  onconfirm={async (name) => {
    await invoke('save_query', {
      name,
      sql: saveQuerySql,
      connectionId: saveQueryConnectionId,
      databaseName: saveQueryDatabaseName,
    });
    await app.loadSavedQueries();
  }}
/>

<!-- Database management dialogs -->
<InputDialog
  bind:open={dbCreateOpen}
  title="Create Database"
  label="Database name"
  placeholder="new_database"
  confirmLabel="Create"
  onconfirm={async (name) => {
    await app.createDatabase(dbCreateConnectionId, name);
    await app.refreshDatabases(dbCreateConnectionId);
  }}
/>

<ConfirmDialog
  bind:open={dbDropOpen}
  title="Drop Database"
  description={'Are you sure you want to drop "' + dbDropDatabaseName + '"? This cannot be undone.'}
  confirmLabel="Drop"
  variant="destructive"
  onconfirm={async () => {
    await app.dropDatabase(dbDropConnectionId, dbDropDatabaseName);
    await app.refreshDatabases(dbDropConnectionId);
  }}
/>

<InputDialog
  bind:open={dbRenameOpen}
  title={'Rename "' + dbRenameOldName + '"'}
  label="New name"
  placeholder="new_name"
  confirmLabel="Rename"
  onconfirm={async (newName) => {
    await app.renameDatabase(dbRenameConnectionId, dbRenameOldName, newName);
    await app.refreshDatabases(dbRenameConnectionId);
  }}
/>

<!-- Query timeout dialog -->
<InputDialog
  bind:open={timeoutOpen}
  title="Set Query Timeout"
  description="Timeout in seconds. Leave empty for no timeout."
  label="Seconds"
  placeholder="30"
  initialValue={timeoutInitialValue}
  confirmLabel="Set"
  onconfirm={(value) => {
    const seconds = value ? Number(value) : null;
    const ms = seconds ? seconds * 1000 : null;
    app.updateQueryTabTimeout(timeoutTabId, ms);
  }}
/>

<!-- List picker (database/schema/page-size) -->
<ListPickerDialog
  bind:open={pickerOpen}
  title={pickerTitle}
  items={pickerItems}
  onselect={pickerOnSelect}
/>

<!-- Export dialog (from command palette) -->
{#if exportOpen}
  <ExportDialog
    bind:open={exportOpen}
    savedConnectionId={exportConnectionId}
    databaseName={exportDatabaseName}
    schema={exportSchema}
    table={exportTable}
    whereClause={exportWhereClause}
  />
{/if}

<!-- Restore dialog (from command palette) -->
<RestoreDialog
  bind:open={restoreOpen}
  savedConnectionId={restoreConnectionId}
  databaseName={restoreDatabaseName}
/>
