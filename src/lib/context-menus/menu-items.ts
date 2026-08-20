import type { MenuContext, MenuEntry } from './types';

// ── Category folder menus ──

export function tablesFolderMenuItems(): MenuEntry[] {
  return [
    { id: 'create', label: 'Create Table...', when: c => c.capabilities?.sql !== false },
  ];
}

export function viewsFolderMenuItems(): MenuEntry[] {
  return [
    { id: 'create', label: 'Create View...', when: c => c.capabilities?.sql !== false },
  ];
}

export function materializedViewsFolderMenuItems(): MenuEntry[] {
  return [
    { id: 'create', label: 'Create Materialized View...', when: c => c.capabilities?.sql !== false },
  ];
}

export function functionsFolderMenuItems(): MenuEntry[] {
  return [
    { id: 'create', label: 'Create Function...', when: c => c.capabilities?.sql !== false },
  ];
}

export function sequencesFolderMenuItems(): MenuEntry[] {
  return [
    { id: 'create', label: 'Create Sequence...', when: c => c.capabilities?.sql !== false },
  ];
}

export function indexesFolderMenuItems(): MenuEntry[] {
  return [
    { id: 'create', label: 'Create Index...', when: c => c.capabilities?.sql !== false },
  ];
}

// ── Tree node menus ──

export function tableMenuItems(): MenuEntry[] {
  return [
    { id: 'open-data', label: 'Open Data' },
    { id: 'view-structure', label: 'View Structure' },
    { id: 'view-erd', label: 'View ERD', when: c => c.capabilities?.introspection !== false },
    { id: 'new-query', label: 'New Query', when: c => c.capabilities?.sql !== false },
    { kind: 'separator' },
    { id: 'export', label: 'Export Table...', when: c => c.capabilities?.export !== false },
    { id: 'restore', label: 'Restore from SQL...', when: c => c.capabilities?.restore === true },
    { id: 'sql-create', label: 'SQL: Create', when: c => c.capabilities?.sql !== false },
    { id: 'duplicate', label: 'Duplicate Table...', when: c => c.capabilities?.sql !== false },
    { id: 'rename', label: 'Rename Table...', when: c => c.capabilities?.sql !== false },
    { kind: 'separator', when: c => c.capabilities?.sql !== false },
    { id: 'truncate', label: 'Truncate Table...', variant: 'destructive', when: c => c.capabilities?.sql !== false },
    { id: 'drop', label: 'Drop Table...', variant: 'destructive', when: c => c.capabilities?.sql !== false },
  ];
}

export function viewMenuItems(): MenuEntry[] {
  return [
    { id: 'open-data', label: 'Open Data' },
    { id: 'view-structure', label: 'View Structure', when: c => c.capabilities?.introspection !== false },
    { id: 'new-query', label: 'New Query', when: c => c.capabilities?.sql !== false },
    { kind: 'separator' },
    { id: 'edit', label: 'Edit View...', when: c => c.capabilities?.sql !== false },
    { id: 'copy-name', label: 'Copy Qualified Name' },
    { kind: 'separator', when: c => c.capabilities?.sql !== false },
    { id: 'drop', label: 'Drop View...', variant: 'destructive', when: c => c.capabilities?.sql !== false },
  ];
}

export function materializedViewMenuItems(): MenuEntry[] {
  return [
    { id: 'open-data', label: 'Open Data' },
    { id: 'view-structure', label: 'View Structure', when: c => c.capabilities?.introspection !== false },
    { id: 'refresh', label: 'Refresh' },
    { id: 'new-query', label: 'New Query', when: c => c.capabilities?.sql !== false },
    { kind: 'separator' },
    { id: 'edit', label: 'Edit Materialized View...', when: c => c.capabilities?.sql !== false },
    { id: 'copy-name', label: 'Copy Qualified Name' },
    { kind: 'separator', when: c => c.capabilities?.sql !== false },
    { id: 'drop', label: 'Drop Materialized View...', variant: 'destructive', when: c => c.capabilities?.sql !== false },
  ];
}

export function functionMenuItems(): MenuEntry[] {
  return [
    { id: 'view-structure', label: 'View Definition', when: c => c.capabilities?.introspection !== false },
    { id: 'edit', label: 'Edit Function...', when: c => c.capabilities?.sql !== false },
    { id: 'copy-name', label: 'Copy Name' },
    { kind: 'separator', when: c => c.capabilities?.sql !== false },
    { id: 'drop', label: 'Drop Function...', variant: 'destructive', when: c => c.capabilities?.sql !== false },
  ];
}

export function sequenceMenuItems(): MenuEntry[] {
  return [
    { id: 'edit', label: 'Edit Sequence...', when: c => c.capabilities?.sql !== false },
    { id: 'copy-name', label: 'Copy Name' },
    { kind: 'separator', when: c => c.capabilities?.sql !== false },
    { id: 'reset', label: 'Reset Sequence', when: c => c.capabilities?.sql !== false },
    { kind: 'separator', when: c => c.capabilities?.sql !== false },
    { id: 'drop', label: 'Drop Sequence...', variant: 'destructive', when: c => c.capabilities?.sql !== false },
  ];
}

export function indexMenuItems(): MenuEntry[] {
  return [
    { id: 'copy-name', label: 'Copy Name' },
    { kind: 'separator', when: c => c.capabilities?.sql !== false },
    { id: 'reindex', label: 'Reindex', when: c => c.capabilities?.sql !== false },
    { kind: 'separator', when: c => c.capabilities?.sql !== false },
    { id: 'drop', label: 'Drop Index...', variant: 'destructive', when: c => c.capabilities?.sql !== false },
  ];
}

export function foreignTableMenuItems(): MenuEntry[] {
  return [
    { id: 'open-data', label: 'Open Data' },
    { id: 'view-structure', label: 'View Structure', when: c => c.capabilities?.introspection !== false },
    { id: 'new-query', label: 'New Query', when: c => c.capabilities?.sql !== false },
    { kind: 'separator' },
    { id: 'copy-name', label: 'Copy Name' },
    { kind: 'separator', when: c => c.capabilities?.sql !== false },
    { id: 'drop', label: 'Drop Foreign Table...', variant: 'destructive', when: c => c.capabilities?.sql !== false },
  ];
}

// ── Database & schema menus (inside DatabaseNode) ──

export function databaseMenuItems(ctx: MenuContext): MenuEntry[] {
  const connected = () => ctx.isDbConnected === true;
  return [
    // Work
    { id: 'new-query', label: 'New Query', when: () => connected() && ctx.capabilities?.sql !== false },
    { id: 'create-schema', label: 'Create Schema...', when: () => connected() && ctx.capabilities?.sql !== false },
    { kind: 'separator', when: () => connected() },
    // Maintenance
    { id: 'refresh', label: 'Refresh', when: () => connected() },
    { id: 'export-db', label: 'Export Database...', when: () => connected() && ctx.capabilities?.export !== false && ctx.capabilities?.sql !== false },
    { id: 'restore', label: 'Restore from SQL...', when: () => connected() && ctx.capabilities?.restore === true },
    { kind: 'separator' },
    // Connection
    { id: 'disconnect', label: 'Disconnect', when: () => connected() },
    { id: 'connect', label: 'Connect', when: () => !connected() },
    // Database ops
    { kind: 'separator', when: () => ctx.capabilities?.multi_database === true },
    { id: 'rename-db', label: 'Rename Database...', when: () => ctx.capabilities?.multi_database === true },
    { id: 'edit-db', label: 'Edit Database...', when: () => ctx.capabilities?.multi_database === true },
    { kind: 'separator', when: () => ctx.capabilities?.multi_database === true },
    { id: 'drop-db', label: 'Drop Database', variant: 'destructive', when: () => ctx.capabilities?.multi_database === true },
  ];
}

export function schemaMenuItems(ctx: MenuContext): MenuEntry[] {
  return [
    { id: 'new-query', label: 'New Query', when: () => ctx.capabilities?.sql !== false },
    { id: 'view-erd', label: 'View ERD', when: () => ctx.capabilities?.introspection !== false },
    { kind: 'separator' },
    { id: 'refresh', label: 'Refresh' },
    { id: 'export-schema', label: 'Export Schema...', when: () => ctx.capabilities?.export !== false && ctx.capabilities?.sql !== false },
    { id: 'restore', label: 'Restore from SQL...', when: () => ctx.capabilities?.restore === true },
    { kind: 'separator', when: () => ctx.capabilities?.sql !== false },
    { id: 'create-schema', label: 'Create Schema...', when: () => ctx.capabilities?.sql !== false },
    { id: 'rename-schema', label: 'Rename Schema...', when: () => ctx.capabilities?.sql !== false },
    { kind: 'separator', when: () => ctx.capabilities?.sql !== false },
    { id: 'drop-schema', label: 'Drop Schema...', variant: 'destructive', when: () => ctx.capabilities?.sql !== false },
  ];
}

// ── Connection menus ──

export function connectionTreeMenuItems(ctx: MenuContext): MenuEntry[] {
  const connected = () => ctx.isConnected === true;
  return [
    // Server work
    { id: 'new-query', label: 'New Query', when: () => connected() && ctx.capabilities?.sql !== false },
    { id: 'refresh', label: 'Refresh', when: () => connected() },
    // Database creation
    { kind: 'separator', when: () => connected() && ctx.capabilities?.multi_database === true },
    { id: 'create-db', label: 'New Database', when: () => connected() && ctx.capabilities?.multi_database === true },
    // SQLite maintenance
    { kind: 'separator', when: () => connected() && ctx.engineType === 'sqlite' },
    { id: 'vacuum', label: 'Vacuum', when: () => connected() && ctx.engineType === 'sqlite' },
    { id: 'integrity-check', label: 'Integrity Check', when: () => connected() && ctx.engineType === 'sqlite' },
    // Connection
    { kind: 'separator', when: () => connected() },
    { id: 'disconnect', label: 'Disconnect', when: () => connected() },
    { id: 'connect', label: 'Connect', when: () => !connected() },
    // Always
    { kind: 'separator' },
    { id: 'edit', label: 'Edit' },
    { id: 'duplicate', label: 'Duplicate' },
    { id: 'delete', label: 'Delete', variant: 'destructive' },
  ];
}

export function connectionManagerMenuItems(): MenuEntry[] {
  return [
    { id: 'edit', label: 'Edit' },
    { id: 'duplicate', label: 'Duplicate' },
    { kind: 'separator' },
    { id: 'delete', label: 'Delete', variant: 'destructive' },
  ];
}

// ── Query list menu ──

export function savedQueryMenuItems(ctx: MenuContext): MenuEntry[] {
  return [
    { id: 'open', label: 'Open in New Tab' },
    { id: 'copy-sql', label: 'Copy SQL' },
    // Saved query items
    { kind: 'separator', when: () => ctx.isHistory !== true },
    { id: 'edit', label: 'Edit', when: () => ctx.isHistory !== true },
    { id: 'delete', label: 'Delete', variant: 'destructive', when: () => ctx.isHistory !== true },
    // History items
    { kind: 'separator', when: () => ctx.isHistory === true },
    { id: 'save', label: 'Save Query', when: () => ctx.isHistory === true },
  ];
}
