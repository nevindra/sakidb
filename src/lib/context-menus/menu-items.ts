import type { MenuContext, MenuEntry } from './types';

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
    { kind: 'separator', when: c => c.capabilities?.sql !== false },
    { id: 'truncate', label: 'Truncate Table...', variant: 'destructive', when: c => c.capabilities?.sql !== false },
    { id: 'drop', label: 'Drop Table...', variant: 'destructive', when: c => c.capabilities?.sql !== false },
  ];
}

export function viewMenuItems(): MenuEntry[] {
  return [
    { id: 'open-data', label: 'Open Data' },
    { id: 'new-query', label: 'New Query' },
    { kind: 'separator' },
    { id: 'copy-name', label: 'Copy Qualified Name' },
  ];
}

export function materializedViewMenuItems(): MenuEntry[] {
  return [
    { id: 'open-data', label: 'Open Data' },
    { id: 'refresh', label: 'Refresh' },
    { id: 'new-query', label: 'New Query' },
    { kind: 'separator' },
    { id: 'copy-name', label: 'Copy Qualified Name' },
  ];
}

export function functionMenuItems(): MenuEntry[] {
  return [
    { id: 'copy-name', label: 'Copy Name' },
  ];
}

export function objectInfoMenuItems(): MenuEntry[] {
  return [
    { id: 'copy-name', label: 'Copy Name' },
  ];
}

// ── Database & schema menus (inside DatabaseNode) ──

export function databaseMenuItems(ctx: MenuContext): MenuEntry[] {
  return [
    // Connected items
    { id: 'new-query', label: 'New Query', when: () => ctx.isDbConnected === true && ctx.capabilities?.sql !== false },
    { kind: 'separator', when: () => ctx.isDbConnected === true && ctx.capabilities?.sql !== false },
    { id: 'restore', label: 'Restore from SQL...', when: () => ctx.isDbConnected === true && ctx.capabilities?.restore === true },
    { kind: 'separator', when: () => ctx.isDbConnected === true && ctx.capabilities?.restore === true },
    { id: 'refresh', label: 'Refresh', when: () => ctx.isDbConnected === true },
    { id: 'disconnect', label: 'Disconnect', when: () => ctx.isDbConnected === true },
    // Disconnected
    { id: 'connect', label: 'Connect', when: () => ctx.isDbConnected !== true },
    // Multi-database ops
    { kind: 'separator', when: () => ctx.capabilities?.multi_database === true },
    { id: 'create-db', label: 'New Database', when: () => ctx.capabilities?.multi_database === true },
    { id: 'rename-db', label: 'Rename Database', when: () => ctx.capabilities?.multi_database === true },
    { kind: 'separator', when: () => ctx.capabilities?.multi_database === true },
    { id: 'drop-db', label: 'Drop Database', variant: 'destructive', when: () => ctx.capabilities?.multi_database === true },
  ];
}

export function schemaMenuItems(ctx: MenuContext): MenuEntry[] {
  return [
    { id: 'view-erd', label: 'View ERD', when: () => ctx.capabilities?.introspection !== false },
    { id: 'new-query', label: 'New Query', when: () => ctx.capabilities?.sql !== false },
    { kind: 'separator', when: () => ctx.capabilities?.restore === true },
    { id: 'restore', label: 'Restore from SQL...', when: () => ctx.capabilities?.restore === true },
  ];
}

// ── Connection menus ──

export function connectionTreeMenuItems(ctx: MenuContext): MenuEntry[] {
  return [
    // Connected items
    { id: 'new-query', label: 'New Query', when: () => ctx.isConnected === true && ctx.capabilities?.sql !== false },
    { kind: 'separator', when: () => ctx.isConnected === true && ctx.capabilities?.sql !== false },
    { id: 'vacuum', label: 'Vacuum', when: () => ctx.isConnected === true && ctx.engineType === 'sqlite' },
    { id: 'integrity-check', label: 'Integrity Check', when: () => ctx.isConnected === true && ctx.engineType === 'sqlite' },
    { kind: 'separator', when: () => ctx.isConnected === true && ctx.engineType === 'sqlite' },
    { id: 'disconnect', label: 'Disconnect', when: () => ctx.isConnected === true },
    // Disconnected
    { id: 'connect', label: 'Connect', when: () => ctx.isConnected !== true },
    // Always
    { kind: 'separator' },
    { id: 'edit', label: 'Edit' },
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
