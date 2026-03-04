import type { LayoutNode, TabGroup, SplitNode } from '$lib/types';
import { generateId } from './shared.svelte';

// ── State ──

const defaultGroup: TabGroup = { type: 'tab-group', id: generateId(), tabIds: [], activeTabId: null };
let layoutRoot = $state<LayoutNode>(defaultGroup);
let activeGroupId = $state<string>(defaultGroup.id);

// Callback to notify tabs store when a tab becomes active via layout operations
let onTabActivated: ((tabId: string) => void) | null = null;
export function registerLayoutTabHandler(handler: (tabId: string) => void) {
  onTabActivated = handler;
}

function notifyTabActivated(tabId: string | null) {
  if (tabId) onTabActivated?.(tabId);
}

// ── Persistence ──

const STORAGE_KEY = 'sakidb:layout';
let saveTimer: ReturnType<typeof setTimeout> | null = null;

function scheduleSave() {
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify({ version: 1, root: layoutRoot, activeGroupId }));
    } catch { /* quota exceeded — ignore */ }
  }, 300);
}

// ── Tree helpers ──

function findGroupInNode(node: LayoutNode, groupId: string): TabGroup | null {
  if (node.type === 'tab-group') return node.id === groupId ? node : null;
  return findGroupInNode(node.first, groupId) ?? findGroupInNode(node.second, groupId);
}

function findParentSplit(node: LayoutNode, childId: string, parent: SplitNode | null = null): { parent: SplitNode; which: 'first' | 'second' } | null {
  if (node.type === 'tab-group') {
    return node.id === childId && parent ? { parent, which: parent.first === node ? 'first' : 'second' } : null;
  }
  if (node.id === childId && parent) {
    return { parent, which: parent.first === node ? 'first' : 'second' };
  }
  return findParentSplit(node.first, childId, node) ?? findParentSplit(node.second, childId, node);
}

function replaceNodeInTree(root: LayoutNode, targetId: string, replacement: LayoutNode): LayoutNode {
  if (root.id === targetId) return replacement;
  if (root.type === 'tab-group') return root;
  return {
    ...root,
    first: replaceNodeInTree(root.first, targetId, replacement),
    second: replaceNodeInTree(root.second, targetId, replacement),
  };
}

function findGroupContainingTab(node: LayoutNode, tabId: string): TabGroup | null {
  if (node.type === 'tab-group') return node.tabIds.includes(tabId) ? node : null;
  return findGroupContainingTab(node.first, tabId) ?? findGroupContainingTab(node.second, tabId);
}

function allGroups(node: LayoutNode): TabGroup[] {
  if (node.type === 'tab-group') return [node];
  return [...allGroups(node.first), ...allGroups(node.second)];
}

function allVisibleTabIds(node: LayoutNode): Set<string> {
  const groups = allGroups(node);
  const ids = new Set<string>();
  for (const g of groups) {
    if (g.activeTabId) ids.add(g.activeTabId);
  }
  return ids;
}

// ── Read access ──

export function getLayoutRoot(): LayoutNode { return layoutRoot; }
export function getActiveGroupId(): string { return activeGroupId; }

export function getActiveGroup(): TabGroup | null {
  return findGroupInNode(layoutRoot, activeGroupId);
}

export function getVisibleTabIds(): Set<string> {
  return allVisibleTabIds(layoutRoot);
}

export function getGroupContainingTab(tabId: string): TabGroup | null {
  return findGroupContainingTab(layoutRoot, tabId);
}

export function getAllGroups(): TabGroup[] {
  return allGroups(layoutRoot);
}

// ── Write access ──

export function setActiveGroup(groupId: string) {
  activeGroupId = groupId;
  const group = findGroupInNode(layoutRoot, groupId);
  notifyTabActivated(group?.activeTabId ?? null);
  scheduleSave();
}

export function setActiveTabInGroup(groupId: string, tabId: string) {
  const group = findGroupInNode(layoutRoot, groupId);
  if (!group) return;
  group.activeTabId = tabId;
  activeGroupId = groupId;
  notifyTabActivated(tabId);
  scheduleSave();
}

export function addTabToActiveGroup(tabId: string) {
  const group = findGroupInNode(layoutRoot, activeGroupId);
  if (!group) {
    // Fallback: add to first group
    const groups = allGroups(layoutRoot);
    if (groups.length > 0) {
      groups[0].tabIds.push(tabId);
      groups[0].activeTabId = tabId;
      activeGroupId = groups[0].id;
    }
  } else {
    group.tabIds.push(tabId);
    group.activeTabId = tabId;
  }
  scheduleSave();
}

export function removeTabFromGroup(tabId: string) {
  const group = findGroupContainingTab(layoutRoot, tabId);
  if (!group) return;

  group.tabIds = group.tabIds.filter(id => id !== tabId);

  if (group.activeTabId === tabId) {
    group.activeTabId = group.tabIds.length > 0 ? group.tabIds[group.tabIds.length - 1] : null;
  }

  // Auto-collapse empty groups (keep at least the root)
  if (group.tabIds.length === 0) {
    const groups = allGroups(layoutRoot);
    if (groups.length > 1) {
      collapseEmptyGroup(group.id);
    }
  }

  // Notify new active tab in the group (if tab was active and group still has tabs)
  notifyTabActivated(group.activeTabId);
  scheduleSave();
}

export function removeTabsByIds(tabIds: Set<string>) {
  const groups = allGroups(layoutRoot);
  const emptyGroupIds: string[] = [];

  for (const group of groups) {
    group.tabIds = group.tabIds.filter(id => !tabIds.has(id));
    if (group.activeTabId && tabIds.has(group.activeTabId)) {
      group.activeTabId = group.tabIds.length > 0 ? group.tabIds[group.tabIds.length - 1] : null;
    }
    if (group.tabIds.length === 0) {
      emptyGroupIds.push(group.id);
    }
  }

  // Collapse empty groups (keep at least one)
  for (const groupId of emptyGroupIds) {
    const groups = allGroups(layoutRoot);
    if (groups.length <= 1) break;
    collapseEmptyGroup(groupId);
  }

  scheduleSave();
}

// ── Split operations ──

export function splitGroup(groupId: string, direction: 'horizontal' | 'vertical', tabId?: string) {
  const group = findGroupInNode(layoutRoot, groupId);
  if (!group) return;

  const newGroup: TabGroup = { type: 'tab-group', id: generateId(), tabIds: [], activeTabId: null };

  if (tabId && group.tabIds.includes(tabId)) {
    group.tabIds = group.tabIds.filter(id => id !== tabId);
    if (group.activeTabId === tabId) {
      group.activeTabId = group.tabIds.length > 0 ? group.tabIds[group.tabIds.length - 1] : null;
    }
    newGroup.tabIds = [tabId];
    newGroup.activeTabId = tabId;
  }

  // If source group is now empty, just replace it with the new group (no split needed)
  if (group.tabIds.length === 0) {
    layoutRoot = replaceNodeInTree(layoutRoot, groupId, newGroup);
    activeGroupId = newGroup.id;
    notifyTabActivated(newGroup.activeTabId);
    scheduleSave();
    return;
  }

  const splitNode: SplitNode = {
    type: 'split',
    id: generateId(),
    direction,
    ratio: 0.5,
    first: { ...group },
    second: newGroup,
  };

  layoutRoot = replaceNodeInTree(layoutRoot, groupId, splitNode);
  activeGroupId = newGroup.id;
  notifyTabActivated(newGroup.activeTabId);
  notifyTabActivated(group.activeTabId);
  scheduleSave();
}

export function resizeSplit(splitId: string, ratio: number) {
  const clamped = Math.max(0.1, Math.min(0.9, ratio));
  function update(node: LayoutNode): LayoutNode {
    if (node.type === 'tab-group') return node;
    if (node.id === splitId) {
      return { ...node, ratio: clamped, first: node.first, second: node.second };
    }
    return { ...node, first: update(node.first), second: update(node.second) };
  }
  layoutRoot = update(layoutRoot);
  scheduleSave();
}

export function moveTab(tabId: string, fromGroupId: string, toGroupId: string, insertIndex?: number) {
  if (fromGroupId === toGroupId) return;

  const fromGroup = findGroupInNode(layoutRoot, fromGroupId);
  const toGroup = findGroupInNode(layoutRoot, toGroupId);
  if (!fromGroup || !toGroup) return;

  fromGroup.tabIds = fromGroup.tabIds.filter(id => id !== tabId);
  if (fromGroup.activeTabId === tabId) {
    fromGroup.activeTabId = fromGroup.tabIds.length > 0 ? fromGroup.tabIds[fromGroup.tabIds.length - 1] : null;
  }

  if (insertIndex !== undefined) {
    toGroup.tabIds.splice(insertIndex, 0, tabId);
  } else {
    toGroup.tabIds.push(tabId);
  }
  toGroup.activeTabId = tabId;
  activeGroupId = toGroupId;
  notifyTabActivated(tabId);

  // Collapse source if empty
  if (fromGroup.tabIds.length === 0) {
    const groups = allGroups(layoutRoot);
    if (groups.length > 1) {
      collapseEmptyGroup(fromGroupId);
    }
  }

  scheduleSave();
}

export function reorderTab(groupId: string, tabId: string, newIndex: number) {
  const group = findGroupInNode(layoutRoot, groupId);
  if (!group) return;
  group.tabIds = group.tabIds.filter(id => id !== tabId);
  group.tabIds.splice(newIndex, 0, tabId);
  scheduleSave();
}

// ── Collapse ──

/** State for animated collapse. Components check this to apply transition. */
let collapsingGroupId = $state<string | null>(null);
export function getCollapsingGroupId(): string | null { return collapsingGroupId; }

export function collapseEmptyGroup(groupId: string) {
  if (layoutRoot.type === 'tab-group') return; // can't collapse root

  const info = findParentSplit(layoutRoot, groupId);
  if (!info) return;

  const { parent, which } = info;
  const survivor = which === 'first' ? parent.second : parent.first;

  // If the active group was the one being removed, switch to the survivor
  if (activeGroupId === groupId) {
    const survivorGroups = survivor.type === 'tab-group' ? [survivor] : allGroups(survivor);
    if (survivorGroups.length > 0) {
      activeGroupId = survivorGroups[0].id;
    }
  }

  layoutRoot = replaceNodeInTree(layoutRoot, parent.id, survivor);
  scheduleSave();
}

export function startCollapse(groupId: string) {
  collapsingGroupId = groupId;
}

export function finishCollapse(groupId: string) {
  collapsingGroupId = null;
  collapseEmptyGroup(groupId);
}

// ── Persistence restore ──

export function restoreLayout(validTabIds: Set<string>) {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return;
    const data = JSON.parse(raw);
    if (data?.version !== 1 || !data.root) return;

    // Prune invalid tab IDs
    function pruneNode(node: LayoutNode): LayoutNode | null {
      if (node.type === 'tab-group') {
        const tabIds = node.tabIds.filter(id => validTabIds.has(id));
        const activeTabId = node.activeTabId && validTabIds.has(node.activeTabId) ? node.activeTabId : (tabIds[0] ?? null);
        return { ...node, tabIds, activeTabId };
      }
      const first = pruneNode(node.first);
      const second = pruneNode(node.second);
      if (!first || !second) return first ?? second ?? null;
      // Collapse single-child splits
      if (first.type === 'tab-group' && first.tabIds.length === 0) return second;
      if (second.type === 'tab-group' && second.tabIds.length === 0) return first;
      return { ...node, first, second };
    }

    const pruned = pruneNode(data.root);
    if (pruned) {
      layoutRoot = pruned;
      const groups = allGroups(layoutRoot);
      activeGroupId = (data.activeGroupId && findGroupInNode(layoutRoot, data.activeGroupId))
        ? data.activeGroupId
        : (groups[0]?.id ?? defaultGroup.id);
    }
  } catch { /* corrupt data — use default */ }
}

export function resetLayout(tabIds: string[]) {
  const group: TabGroup = {
    type: 'tab-group',
    id: generateId(),
    tabIds,
    activeTabId: tabIds[0] ?? null,
  };
  layoutRoot = group;
  activeGroupId = group.id;
  scheduleSave();
}
