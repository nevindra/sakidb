// Barrel re-export — composes getAppState() from domain modules.
// Consumers import { getAppState } from '$lib/stores' and get the same API.

import { getError, clearError } from './shared.svelte';
import {
  getSearchIndex,
  searchTree,
  hasDescendantMatch,
} from './search.svelte';
import {
  getSavedConnections,
  getSavedConnection,
  getActiveConnections,
  getConnectingIds,
  getEditDialogConnectionId,
  hasActiveConnections,
  getRuntimeId,
  getCapabilities,
  loadConnections,
  saveConnection,
  deleteConnection,
  updateConnection,
  testConnection,
  isConnected,
  isConnecting,
  getDatabases,
  getActiveDatabase,
  getRuntimeConnectionId,
  getSchemas,
  isDatabaseConnected,
  isDatabaseConnecting,
  connectToDatabase,
  connectToSpecificDatabase,
  disconnectFromDatabase,
  disconnectSpecificDatabase,
  dropDatabase,
  createDatabase,
  renameDatabase,
  refreshDatabases,
  loadTables,
  loadColumns,
  loadViews,
  loadMaterializedViews,
  loadFunctions,
  loadSequences,
  loadIndexes,
  loadForeignTables,
  openEditDialog,
  closeEditDialog,
  getOracleDriverStatus,
  getOracleDownloadProgress,
  getIsOracleDownloading,
  checkOracleDriverStatus,
  downloadOracleDriver,
} from './connections.svelte';
import {
  getTabs,
  getActiveTabId,
  getActiveTab,
  selectedObjectPath,
  setActiveTab,
  closeTab,
  getTabById,
  registerTabFocusHandler,
} from './tabs.svelte';
import {
  openDataTab,
  loadDataTab,
  buildDataTabQuery,
  filterToSql,
  updateDataTabFilters,
  updateDataTabPageSize,
} from './data-tab.svelte';
import {
  getSavedQueries,
  getQueryHistory,
  getQueryTimeoutSeconds,
  openQueryTab,
  executeQueryInTab,
  updateQueryTabContent,
  setActiveResultIndex,
  updateQueryTabTimeout,
  cancelQuery,
  switchQueryTabDatabase,
  switchQueryTabSchema,
  getSchemaCompletionData,
  getCompletionBundle,
  getTableColumnsForCompletion,
  setQueryTimeout,
  loadSavedQueries,
  loadQueryHistory,
  saveFromHistory,
  updateSavedQuery,
  deleteSavedQuery,
  clearHistory,
} from './query-tab.svelte';
import {
  openStructureTab,
  loadStructureTab,
  executeDdl,
  openErdTab,
  loadErdTab,
  loadTriggers,
  loadForeignKeys,
  loadCheckConstraints,
  loadUniqueConstraints,
  loadPartitionInfo,
  getCreateTableSql,
  loadProfilingData,
} from './structure-tab.svelte';
import {
  exportTableCsv,
  restoreFromSql,
  cancelRestore,
  cancelExport,
  exportTableSql,
} from './exports.svelte';
import {
  getUpdate,
  isDownloading,
  getDownloaded,
  getContentLength,
  isReadyToInstall,
  isDismissed,
  isChecking,
  dismissBanner,
  checkForUpdate,
  downloadAndInstall,
  restartApp,
} from './updater.svelte';
import {
  getLayoutRoot,
  getActiveGroupId,
  getActiveGroup,
  splitGroup,
  resizeSplit,
  moveTab,
  reorderTab,
  setActiveGroup,
  setActiveTabInGroup,
  restoreLayout,
  resetLayout,
} from './layout.svelte';

export function getAppState() {
  return {
    // ── Getters ──
    get savedConnections() { return getSavedConnections(); },
    get error() { return getError(); },
    get activeConnections() { return getActiveConnections(); },
    get tabs() { return getTabs(); },
    get activeTabId() { return getActiveTabId(); },
    get activeTab() { return getActiveTab(); },
    get hasActiveConnections() { return hasActiveConnections(); },
    get editDialogConnectionId() { return getEditDialogConnectionId(); },
    get connectingIds() { return getConnectingIds(); },
    get queryTimeoutSeconds() { return getQueryTimeoutSeconds(); },
    get savedQueries() { return getSavedQueries(); },
    get queryHistory() { return getQueryHistory(); },
    get searchIndex() { return getSearchIndex(); },
    get selectedObjectPath() { return selectedObjectPath(); },
    get oracleDriverStatus() { return getOracleDriverStatus(); },
    get oracleDownloadProgress() { return getOracleDownloadProgress(); },
    get isOracleDownloading() { return getIsOracleDownloading(); },

    // ── Layout ──
    get layoutRoot() { return getLayoutRoot(); },
    get activeGroupId() { return getActiveGroupId(); },
    get activeGroup() { return getActiveGroup(); },
    splitGroup,
    resizeSplit,
    moveTab: moveTab,
    reorderTab,
    setActiveGroup,
    setActiveTabInGroup,
    getTabById,

    // ── Search ──
    searchTree,
    hasDescendantMatch,

    // ── Connection lookup ──
    getSavedConnection,
    checkOracleDriverStatus,
    downloadOracleDriver,

    // ── Connection CRUD ──
    loadConnections,
    saveConnection,
    deleteConnection,
    updateConnection,
    testConnection,

    // ── Connect / Disconnect ──
    isConnected,
    isConnecting,
    getCapabilities,
    getDatabases,
    getActiveDatabase,
    getRuntimeConnectionId,
    getSchemas,
    isDatabaseConnected,
    isDatabaseConnecting,
    connectToDatabase,
    connectToSpecificDatabase,
    disconnectFromDatabase,
    disconnectSpecificDatabase,

    // ── Database management ──
    dropDatabase,
    createDatabase,
    renameDatabase,
    refreshDatabases,

    // ── Runtime ID helper ──
    _getRuntimeId: getRuntimeId,

    // ── Schema / Table / Column loading ──
    loadTables,
    loadColumns,
    loadViews,
    loadMaterializedViews,
    loadFunctions,
    loadSequences,
    loadIndexes,
    loadForeignTables,

    // ── Tab management ──
    openDataTab,
    openQueryTab,
    openStructureTab,
    openErdTab,
    setActiveTab,
    closeTab,

    // ── Data tab operations ──
    loadDataTab,
    buildDataTabQuery,
    filterToSql,
    updateDataTabFilters,
    updateDataTabPageSize,

    // ── Query tab operations ──
    executeQueryInTab,
    updateQueryTabContent,
    setActiveResultIndex,
    updateQueryTabTimeout,
    cancelQuery,
    switchQueryTabDatabase,
    switchQueryTabSchema,
    getSchemaCompletionData,
    getCompletionBundle,
    getTableColumnsForCompletion,

    // ── Query management ──
    loadSavedQueries,
    loadQueryHistory,
    saveFromHistory,
    updateSavedQuery,
    deleteSavedQuery,
    clearHistory,
    setQueryTimeout,

    // ── Structure / ERD ──
    loadStructureTab,
    loadErdTab,
    loadTriggers,
    loadForeignKeys,
    loadCheckConstraints,
    loadUniqueConstraints,
    loadPartitionInfo,
    getCreateTableSql,
    loadProfilingData,
    executeDdl,

    // ── Export ──
    exportTableCsv,
    restoreFromSql,
    cancelRestore,
    cancelExport,
    exportTableSql,

    // ── UI state ──
    openEditDialog,
    closeEditDialog,
    clearError,

    // ── Updater ──
    get update() { return getUpdate(); },
    get updateDownloading() { return isDownloading(); },
    get updateDownloaded() { return getDownloaded(); },
    get updateContentLength() { return getContentLength(); },
    get updateReadyToInstall() { return isReadyToInstall(); },
    get updateDismissed() { return isDismissed(); },
    get updateChecking() { return isChecking(); },
    dismissUpdateBanner: dismissBanner,
    checkForUpdate,
    downloadAndInstall,
    restartApp,

    // ── Init ──
    async init() {
      registerTabFocusHandler((tab) => {
        if (tab.type === 'data') loadDataTab(tab.id);
        else if (tab.type === 'structure') loadStructureTab(tab.id);
        else if (tab.type === 'erd') loadErdTab(tab.id);
      });
      await loadConnections();
      await loadSavedQueries();
      await loadQueryHistory();

      // Restore persisted layout
      const allTabIds = new Set(getTabs().map(t => t.id));
      restoreLayout(allTabIds);

      // Silent update check on startup
      checkForUpdate();
    },
  };
}
