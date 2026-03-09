import type { Tab, DataTab, QueryTab, ErdTab, StructureTab } from '$lib/types';
import {
  addTabToActiveGroup,
  removeTabFromGroup,
  removeTabsByIds,
  getVisibleTabIds,
  getActiveGroup,
  setActiveTabInGroup,
  getGroupContainingTab,
  registerLayoutTabHandler,
} from './layout.svelte';

// ── State ──

let tabs = $state<Tab[]>([]);
const tabIndex = new Map<string, Tab>();

// ── Read access ──

export function getTabs(): Tab[] { return tabs; }
export function getTabById(id: string): Tab | undefined { return tabIndex.get(id); }

/** The globally active tab — determined by the active group's active tab. */
export function getActiveTab(): Tab | undefined {
  const group = getActiveGroup();
  if (!group?.activeTabId) return undefined;
  return tabIndex.get(group.activeTabId);
}

export function getActiveTabId(): string | null {
  return getActiveGroup()?.activeTabId ?? null;
}

export function selectedObjectPath(): string | null {
  const tab = getActiveTab();
  if (!tab) return null;
  if (tab.type === 'data' || tab.type === 'structure') {
    return `${tab.savedConnectionId}/${tab.databaseName}/${tab.schema}/${tab.table}`;
  }
  if (tab.type === 'erd') {
    return `${tab.savedConnectionId}/${tab.databaseName}/${tab.schema}`;
  }
  if (tab.type === 'query') {
    return `${tab.savedConnectionId}/${tab.databaseName}`;
  }
  return null;
}

// ── Write access ──

export function setActiveTabId(id: string | null) {
  if (!id) return;
  // Find which group contains this tab and activate it there
  const group = getGroupContainingTab(id);
  if (group) {
    setActiveTabInGroup(group.id, id);
    // Reload if evicted
    const tab = tabIndex.get(id);
    if (tab && evictedTabs.has(id)) {
      evictedTabs.delete(id);
      onTabFocus?.(tab);
    }
  }
}

export function addTab(tab: Tab) {
  tabs = [...tabs, tab];
  // Store the reactive proxy from the $state array, not the original object.
  // This ensures getTabById() returns a reactive reference that triggers re-renders.
  tabIndex.set(tab.id, tabs[tabs.length - 1]);
  addTabToActiveGroup(tab.id);
}

export function findTab<T extends Tab>(predicate: (t: Tab) => t is T): T | undefined;
export function findTab(predicate: (t: Tab) => boolean): Tab | undefined;
export function findTab(predicate: (t: Tab) => boolean): Tab | undefined {
  return tabs.find(predicate);
}

// Evicted tab IDs — data/structure/erd tabs whose heavy data was released while in background
const evictedTabs = new Set<string>();

// Callback registered by domain stores to reload evicted tabs on focus
let onTabFocus: ((tab: Tab) => void) | null = null;
export function registerTabFocusHandler(handler: (tab: Tab) => void) {
  onTabFocus = handler;
}

// Listen for layout operations that activate tabs (moveTab, splitGroup, etc.)
registerLayoutTabHandler((tabId: string) => {
  const tab = tabIndex.get(tabId);
  if (tab && evictedTabs.has(tabId)) {
    evictedTabs.delete(tabId);
    onTabFocus?.(tab);
  }
});

// ── Tab operations ──

/** Switch active tab within a group. Evicts tabs no longer visible in any pane. */
export function setActiveTab(tabId: string, groupId?: string) {
  const group = groupId ? undefined : getGroupContainingTab(tabId);
  const gid = groupId ?? group?.id;
  if (!gid) return;

  // Determine which tabs were visible before
  const prevVisible = getVisibleTabIds();

  setActiveTabInGroup(gid, tabId);

  // Determine which tabs are visible now
  const nowVisible = getVisibleTabIds();

  // Evict tabs that were visible before but aren't anymore
  for (const prevId of prevVisible) {
    if (!nowVisible.has(prevId)) {
      evictTab(prevId);
    }
  }

  // Reload if the newly focused tab was evicted
  const next = tabIndex.get(tabId);
  if (next && evictedTabs.has(tabId)) {
    evictedTabs.delete(tabId);
    onTabFocus?.(next);
  }
}

function evictTab(tabId: string) {
  const tab = tabIndex.get(tabId);
  if (!tab) return;
  if (tab.type === 'data' && (tab as DataTab).queryResult) {
    (tab as DataTab).queryResult = null;
    evictedTabs.add(tab.id);
  } else if (tab.type === 'structure') {
    (tab as StructureTab).columns = [];
    (tab as StructureTab).indexes = [];
    (tab as StructureTab).foreignKeys = [];
    (tab as StructureTab).checkConstraints = [];
    (tab as StructureTab).uniqueConstraints = [];
    (tab as StructureTab).triggers = [];
    (tab as StructureTab).partitionInfo = null;
    evictedTabs.add(tab.id);
  } else if (tab.type === 'erd') {
    (tab as ErdTab).tables = [];
    (tab as ErdTab).columns = {};
    (tab as ErdTab).foreignKeys = {};
    evictedTabs.add(tab.id);
  } else if (tab.type === 'query' && (tab as QueryTab).queryResults.length > 0) {
    (tab as QueryTab).queryResults = [];
  }
}

export function closeTab(tabId: string) {
  const tab = tabIndex.get(tabId);
  // Release heavy data
  if (tab && tab.type === 'query') {
    (tab as QueryTab).queryResults = [];
  } else if (tab && tab.type === 'data') {
    (tab as DataTab).queryResult = null;
  }

  // Remove from layout tree
  removeTabFromGroup(tabId);

  // Remove from index and flat array
  tabIndex.delete(tabId);
  tabs = tabs.filter(t => t.id !== tabId);
}

// ── Cross-module helpers (called by connections on disconnect) ──

function releaseTabData(tab: Tab) {
  if (tab.type === 'query') {
    (tab as QueryTab).queryResults = [];
  } else if (tab.type === 'data') {
    (tab as DataTab).queryResult = null;
  }
}

export function closeTabsByConnection(savedConnectionId: string) {
  const removedIds = new Set<string>();
  for (const tab of tabs) {
    if (tab.savedConnectionId === savedConnectionId) {
      releaseTabData(tab);
      removedIds.add(tab.id);
      tabIndex.delete(tab.id);
    }
  }
  tabs = tabs.filter(t => t.savedConnectionId !== savedConnectionId);
  if (removedIds.size > 0) {
    removeTabsByIds(removedIds);
  }
}

export function closeTabsByRuntimeId(runtimeConnectionId: string) {
  const removedIds = new Set<string>();
  for (const tab of tabs) {
    if (tab.runtimeConnectionId === runtimeConnectionId) {
      releaseTabData(tab);
      removedIds.add(tab.id);
      tabIndex.delete(tab.id);
    }
  }
  tabs = tabs.filter(t => t.runtimeConnectionId !== runtimeConnectionId);
  if (removedIds.size > 0) {
    removeTabsByIds(removedIds);
  }
}
