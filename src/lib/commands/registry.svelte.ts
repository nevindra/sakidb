import { invoke } from '@tauri-apps/api/core';
import { commandDefinitions } from './definitions';
import type { Command, CommandContext, CommandDefinition } from './types';

// ── Context state ──

let activeContexts = $state<Set<CommandContext>>(new Set(['global']));

export function setContexts(contexts: CommandContext[]) {
  activeContexts = new Set(['global', ...contexts]);
}

export function getActiveContexts(): Set<CommandContext> {
  return activeContexts;
}

// ── Action registry ──

const actions = new Map<string, () => void | Promise<void>>();

export function registerAction(commandId: string, action: () => void | Promise<void>) {
  actions.set(commandId, action);
}

export function registerActions(entries: Record<string, () => void | Promise<void>>) {
  for (const [id, action] of Object.entries(entries)) {
    actions.set(id, action);
  }
}

// ── Keybinding overrides ──

let overrides = $state<Map<string, string | null>>(new Map());

export async function loadKeybindingOverrides() {
  const rows: [string, string | null][] = await invoke('get_keybinding_overrides');
  overrides = new Map(rows);
}

export async function setKeybinding(commandId: string, keybinding: string | null) {
  await invoke('set_keybinding', { commandId, keybinding });
  overrides.set(commandId, keybinding);
  rebuildKeybindingMap();
}

export async function resetKeybinding(commandId: string) {
  await invoke('reset_keybinding', { commandId });
  overrides.delete(commandId);
  rebuildKeybindingMap();
}

export async function resetAllKeybindings() {
  await invoke('reset_all_keybindings');
  overrides = new Map();
  rebuildKeybindingMap();
}

// ── Resolved keybinding for a command ──

export function getKeybinding(commandId: string): string | null {
  if (overrides.has(commandId)) return overrides.get(commandId) ?? null;
  const def = commandDefinitions.find(d => d.id === commandId);
  return def?.defaultKeybinding ?? null;
}

// ── Coming soon tracking ──

const comingSoonIds = new Set<string>();

export function markComingSoon(commandIds: string[]) {
  for (const id of commandIds) comingSoonIds.add(id);
}

// ── Command list (reactive) ──

export function getCommands(): Command[] {
  return commandDefinitions.map(def => ({
    ...def,
    action: actions.get(def.id) ?? (() => {}),
    enabled: def.contexts.some(c => activeContexts.has(c)),
    comingSoon: comingSoonIds.has(def.id),
  }));
}

export function getActiveCommands(): Command[] {
  return getCommands().filter(c => c.enabled);
}

// ── Recent commands (in-memory) ──

const MAX_RECENT = 5;
let recentCommandIds = $state<string[]>([]);

export function getRecentCommands(): Command[] {
  const commands = getCommands();
  return recentCommandIds
    .map(id => commands.find(c => c.id === id))
    .filter((c): c is Command => c !== undefined && c.enabled);
}

function trackRecent(commandId: string) {
  recentCommandIds = [commandId, ...recentCommandIds.filter(id => id !== commandId)].slice(0, MAX_RECENT);
}

// ── Fuzzy matching ──

export function fuzzyMatch(query: string, text: string): { match: boolean; score: number } {
  const q = query.toLowerCase();
  const t = text.toLowerCase();

  if (!q) return { match: true, score: 0 };

  // Exact substring match gets high score
  if (t.includes(q)) return { match: true, score: 100 + (q.length / t.length) * 50 };

  // Character-order fuzzy match
  let qi = 0;
  let score = 0;
  let lastMatchIdx = -1;

  for (let ti = 0; ti < t.length && qi < q.length; ti++) {
    if (t[ti] === q[qi]) {
      score += 10;
      // Bonus for consecutive matches
      if (lastMatchIdx === ti - 1) score += 5;
      // Bonus for word boundary matches
      if (ti === 0 || t[ti - 1] === ' ' || t[ti - 1] === '.' || t[ti - 1] === '-') score += 8;
      lastMatchIdx = ti;
      qi++;
    }
  }

  if (qi === q.length) return { match: true, score };
  return { match: false, score: 0 };
}

// ── Keybinding map & resolution ──

// Normalized shortcut string → command ID
let keybindingMap = $state<Map<string, string>>(new Map());

// Context priority for conflict resolution (higher = more specific)
const contextPriority: Record<CommandContext, number> = {
  'global': 0,
  'connected': 1,
  'sidebar': 2,
  'query-tab': 3,
  'data-tab': 3,
  'structure-tab': 3,
  'erd-tab': 3,
};

function rebuildKeybindingMap() {
  const map = new Map<string, string>();
  // Build a map of keybinding → {commandId, maxPriority}
  const entries = new Map<string, { commandId: string; priority: number }>();

  for (const def of commandDefinitions) {
    const kb = overrides.has(def.id) ? overrides.get(def.id) : def.defaultKeybinding;
    if (!kb) continue;
    const normalized = normalizeKeybinding(kb);
    const priority = Math.max(...def.contexts.map(c => contextPriority[c]));
    const existing = entries.get(normalized);
    // Higher priority wins; on tie, last one wins
    if (!existing || priority >= existing.priority) {
      entries.set(normalized, { commandId: def.id, priority });
    }
  }

  for (const [kb, { commandId }] of entries) {
    map.set(kb, commandId);
  }

  keybindingMap = map;
}

export function normalizeKeybinding(kb: string): string {
  const parts = kb.split('+').map(p => p.trim());
  const modifiers: string[] = [];
  let key = '';

  for (const part of parts) {
    const lower = part.toLowerCase();
    if (lower === 'ctrl' || lower === 'control') modifiers.push('Ctrl');
    else if (lower === 'shift') modifiers.push('Shift');
    else if (lower === 'alt') modifiers.push('Alt');
    else if (lower === 'meta' || lower === 'cmd' || lower === 'command') modifiers.push('Meta');
    else key = part;
  }

  // Alphabetical modifier order
  modifiers.sort();
  return [...modifiers, key].join('+');
}

function eventToKeybinding(e: KeyboardEvent): string {
  const modifiers: string[] = [];
  if (e.altKey) modifiers.push('Alt');
  if (e.ctrlKey || e.metaKey) modifiers.push('Ctrl');
  if (e.shiftKey) modifiers.push('Shift');

  let key = e.key;
  // Normalize key names
  if (key === ' ') key = 'Space';
  if (key === 'ArrowUp') key = 'ArrowUp';
  if (key === 'ArrowDown') key = 'ArrowDown';
  if (key === 'ArrowLeft') key = 'ArrowLeft';
  if (key === 'ArrowRight') key = 'ArrowRight';
  if (key.length === 1) key = key.toUpperCase();

  // For Tab key, don't include it as modifier
  if (key === 'Tab') return [...modifiers, 'Tab'].join('+');
  if (key === 'Enter') return [...modifiers, 'Enter'].join('+');
  if (key === 'Escape') return [...modifiers, 'Escape'].join('+');
  if (key === 'Backspace') return [...modifiers, 'Backspace'].join('+');
  if (key === 'Delete') return [...modifiers, 'Delete'].join('+');
  if (key === ',') return [...modifiers, ','].join('+');
  if (key === '\\') return [...modifiers, '\\'].join('+');

  // Skip if key is only a modifier
  if (['Control', 'Shift', 'Alt', 'Meta'].includes(key)) return '';

  return [...modifiers, key].join('+');
}

// Global shortcut IDs that should fire even when focus is in an input/textarea
const GLOBAL_OVERRIDE_IDS = new Set([
  'nav.command-palette',
  'app.clear-error',
]);

// ── Global keydown handler ──

let listenerAttached = false;

export function attachGlobalKeyListener() {
  if (listenerAttached) return;
  listenerAttached = true;

  window.addEventListener('keydown', handleGlobalKeydown);
}

export function detachGlobalKeyListener() {
  if (!listenerAttached) return;
  listenerAttached = false;
  window.removeEventListener('keydown', handleGlobalKeydown);
}

function handleGlobalKeydown(e: KeyboardEvent) {
  const target = e.target as HTMLElement;
  const kb = eventToKeybinding(e);
  if (!kb) return;

  const commandId = keybindingMap.get(kb);
  if (!commandId) return;

  const def = commandDefinitions.find(d => d.id === commandId);
  if (!def) return;

  // Skip if inside CodeMirror (let CM handle its own shortcuts)
  const inCodeMirror = target.closest('.cm-editor') !== null;
  if (inCodeMirror && !GLOBAL_OVERRIDE_IDS.has(commandId)) return;

  // Skip if inside input/textarea (except global overrides)
  const inInput = target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable;
  if (inInput && !GLOBAL_OVERRIDE_IDS.has(commandId)) return;

  // Check if command's context is active
  const isActive = def.contexts.some(c => activeContexts.has(c));
  if (!isActive) return;

  // Check if action is registered
  const action = actions.get(commandId);
  if (!action) return;

  e.preventDefault();
  e.stopPropagation();
  trackRecent(commandId);
  action();
}

// ── Execute command by ID ──

export function executeCommand(commandId: string) {
  const action = actions.get(commandId);
  if (action) {
    trackRecent(commandId);
    action();
  }
}

// ── Initialize ──

export async function initRegistry() {
  await loadKeybindingOverrides();
  rebuildKeybindingMap();
  attachGlobalKeyListener();
}

// ── Conflict detection (for settings UI) ──

export function findConflict(keybinding: string, excludeCommandId: string): string | null {
  const normalized = normalizeKeybinding(keybinding);
  for (const def of commandDefinitions) {
    if (def.id === excludeCommandId) continue;
    const kb = overrides.has(def.id) ? overrides.get(def.id) : def.defaultKeybinding;
    if (kb && normalizeKeybinding(kb) === normalized) return def.id;
  }
  return null;
}

// ── Display helper ──

export function formatKeybinding(kb: string): string {
  const isMac = navigator.userAgent.includes('Macintosh');
  if (isMac) {
    return kb
      .replace('Ctrl+', '⌘')
      .replace('Shift+', '⇧')
      .replace('Alt+', '⌥')
      .replace('Meta+', '⌘');
  }
  return kb;
}
