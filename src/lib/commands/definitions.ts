import type { CommandDefinition } from './types';

export const commandDefinitions: CommandDefinition[] = [
  // ── Navigation ──
  { id: 'nav.command-palette', label: 'Command Palette', category: 'Navigation', defaultKeybinding: 'Ctrl+K', contexts: ['global'] },
  { id: 'nav.new-query', label: 'New Query Tab', category: 'Navigation', defaultKeybinding: 'Ctrl+N', contexts: ['connected'] },
  { id: 'nav.close-tab', label: 'Close Tab', category: 'Navigation', defaultKeybinding: 'Ctrl+W', contexts: ['global'] },
  { id: 'nav.next-tab', label: 'Next Tab', category: 'Navigation', defaultKeybinding: 'Ctrl+Tab', contexts: ['global'] },
  { id: 'nav.prev-tab', label: 'Previous Tab', category: 'Navigation', defaultKeybinding: 'Ctrl+Shift+Tab', contexts: ['global'] },
  { id: 'nav.tab-1', label: 'Go to Tab 1', category: 'Navigation', defaultKeybinding: 'Ctrl+1', contexts: ['global'] },
  { id: 'nav.tab-2', label: 'Go to Tab 2', category: 'Navigation', defaultKeybinding: 'Ctrl+2', contexts: ['global'] },
  { id: 'nav.tab-3', label: 'Go to Tab 3', category: 'Navigation', defaultKeybinding: 'Ctrl+3', contexts: ['global'] },
  { id: 'nav.tab-4', label: 'Go to Tab 4', category: 'Navigation', defaultKeybinding: 'Ctrl+4', contexts: ['global'] },
  { id: 'nav.tab-5', label: 'Go to Tab 5', category: 'Navigation', defaultKeybinding: 'Ctrl+5', contexts: ['global'] },
  { id: 'nav.tab-6', label: 'Go to Tab 6', category: 'Navigation', defaultKeybinding: 'Ctrl+6', contexts: ['global'] },
  { id: 'nav.tab-7', label: 'Go to Tab 7', category: 'Navigation', defaultKeybinding: 'Ctrl+7', contexts: ['global'] },
  { id: 'nav.tab-8', label: 'Go to Tab 8', category: 'Navigation', defaultKeybinding: 'Ctrl+8', contexts: ['global'] },
  { id: 'nav.tab-9', label: 'Go to Tab 9', category: 'Navigation', defaultKeybinding: 'Ctrl+9', contexts: ['global'] },
  { id: 'nav.toggle-sidebar', label: 'Toggle Sidebar', category: 'Navigation', defaultKeybinding: 'Ctrl+B', contexts: ['global'] },
  { id: 'nav.focus-sidebar', label: 'Focus Sidebar', category: 'Navigation', defaultKeybinding: 'Ctrl+0', contexts: ['global'] },
  { id: 'nav.focus-editor', label: 'Focus Editor', category: 'Navigation', defaultKeybinding: 'Ctrl+E', contexts: ['global'] },
  { id: 'nav.split-right', label: 'Split Editor Right', category: 'Navigation', defaultKeybinding: 'Ctrl+\\', contexts: ['global'] },
  { id: 'nav.settings', label: 'Open Settings', category: 'Navigation', defaultKeybinding: 'Ctrl+,', contexts: ['global'] },

  // ── Query ──
  { id: 'query.execute', label: 'Execute Query', category: 'Query', defaultKeybinding: 'Ctrl+Enter', contexts: ['query-tab'] },
  { id: 'query.cancel', label: 'Cancel Query', category: 'Query', defaultKeybinding: 'Ctrl+Shift+C', contexts: ['query-tab'] },
  { id: 'query.save', label: 'Save Query', category: 'Query', defaultKeybinding: 'Ctrl+S', contexts: ['query-tab'] },
  { id: 'query.switch-database', label: 'Switch Database', category: 'Query', defaultKeybinding: null, contexts: ['query-tab'] },
  { id: 'query.switch-schema', label: 'Switch Schema', category: 'Query', defaultKeybinding: null, contexts: ['query-tab'] },
  { id: 'query.set-timeout', label: 'Set Query Timeout', category: 'Query', defaultKeybinding: null, contexts: ['query-tab'] },

  // ── Saved Queries ──
  { id: 'saved.list', label: 'Show Saved Queries', category: 'Saved Queries', defaultKeybinding: null, contexts: ['connected'] },
  { id: 'saved.delete', label: 'Delete Saved Query', category: 'Saved Queries', defaultKeybinding: null, contexts: ['connected'] },
  { id: 'saved.clear-history', label: 'Clear Query History', category: 'Saved Queries', defaultKeybinding: null, contexts: ['connected'] },

  // ── Connection ──
  { id: 'conn.new', label: 'New Connection', category: 'Connection', defaultKeybinding: 'Ctrl+Shift+N', contexts: ['global'] },
  { id: 'conn.disconnect', label: 'Disconnect', category: 'Connection', defaultKeybinding: null, contexts: ['connected'] },
  { id: 'conn.refresh', label: 'Refresh Databases', category: 'Connection', defaultKeybinding: null, contexts: ['connected'] },

  // ── Database Management ──
  { id: 'db.create', label: 'Create Database', category: 'Database', defaultKeybinding: null, contexts: ['connected'] },
  { id: 'db.drop', label: 'Drop Database', category: 'Database', defaultKeybinding: null, contexts: ['connected'] },
  { id: 'db.rename', label: 'Rename Database', category: 'Database', defaultKeybinding: null, contexts: ['connected'] },

  // ── Data ──
  { id: 'data.refresh', label: 'Refresh Data', category: 'Data', defaultKeybinding: 'Ctrl+R', contexts: ['data-tab'] },
  { id: 'data.clear-filters', label: 'Clear Filters', category: 'Data', defaultKeybinding: null, contexts: ['data-tab'] },
  { id: 'data.page-size', label: 'Set Page Size', category: 'Data', defaultKeybinding: null, contexts: ['data-tab'] },

  // ── Export / Import ──
  { id: 'export.csv', label: 'Export as CSV', category: 'Export', defaultKeybinding: null, contexts: ['data-tab'] },
  { id: 'export.sql', label: 'Export as SQL', category: 'Export', defaultKeybinding: null, contexts: ['data-tab'] },
  { id: 'import.restore-sql', label: 'Restore from SQL', category: 'Import', defaultKeybinding: null, contexts: ['connected'] },
  { id: 'import.cancel', label: 'Cancel Import/Export', category: 'Import', defaultKeybinding: null, contexts: ['connected'] },

  // ── Structure ──
  { id: 'structure.copy-ddl', label: 'Copy DDL', category: 'Structure', defaultKeybinding: null, contexts: ['structure-tab'] },
  { id: 'structure.profile', label: 'Run Profiling', category: 'Structure', defaultKeybinding: null, contexts: ['structure-tab'] },

  // ── Layout ──
  { id: 'layout.reset', label: 'Reset Layout', category: 'Layout', defaultKeybinding: null, contexts: ['global'] },

  // ── App ──
  { id: 'app.check-update', label: 'Check for Updates', category: 'App', defaultKeybinding: null, contexts: ['global'] },
  { id: 'app.clear-error', label: 'Dismiss Error', category: 'App', defaultKeybinding: 'Escape', contexts: ['global'] },
  { id: 'app.about', label: 'About SakiDB', category: 'App', defaultKeybinding: null, contexts: ['global'] },
];
