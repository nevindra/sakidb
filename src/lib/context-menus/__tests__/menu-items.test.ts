import { describe, it, expect } from 'vitest';
import {
  connectionTreeMenuItems,
  databaseMenuItems,
  schemaMenuItems,
  tableMenuItems,
} from '../menu-items';
import type { MenuContext, MenuEntry, MenuItemDef } from '../types';
import type { EngineCapabilities } from '$lib/types';

function visibleIds(entries: MenuEntry[], ctx: MenuContext): string[] {
  return entries
    .filter(e => !e.when || e.when(ctx))
    .filter((e): e is MenuItemDef => !('kind' in e))
    .map(e => e.id);
}

const pgCaps = {
  sql: true,
  introspection: true,
  export: true,
  restore: true,
  multi_database: true,
  schemas: true,
} as EngineCapabilities;

const sqliteCaps = {
  sql: true,
  introspection: true,
  export: true,
  restore: false,
  multi_database: false,
  schemas: false,
} as EngineCapabilities;

describe('connectionTreeMenuItems (host node)', () => {
  it('shows server-level ops when connected (postgres)', () => {
    const ctx: MenuContext = { capabilities: pgCaps, isConnected: true, engineType: 'postgres' };
    expect(visibleIds(connectionTreeMenuItems(ctx), ctx)).toEqual([
      'new-query', 'refresh', 'create-db', 'disconnect', 'edit', 'duplicate', 'delete',
    ]);
  });

  it('does not offer schema creation at the host level', () => {
    const ctx: MenuContext = { capabilities: pgCaps, isConnected: true, engineType: 'postgres' };
    expect(visibleIds(connectionTreeMenuItems(ctx), ctx)).not.toContain('create-schema');
  });

  it('shows only connect + management when disconnected', () => {
    const ctx: MenuContext = { capabilities: null, isConnected: false, engineType: 'postgres' };
    expect(visibleIds(connectionTreeMenuItems(ctx), ctx)).toEqual([
      'connect', 'edit', 'duplicate', 'delete',
    ]);
  });

  it('shows sqlite maintenance ops but no database creation', () => {
    const ctx: MenuContext = { capabilities: sqliteCaps, isConnected: true, engineType: 'sqlite' };
    const ids = visibleIds(connectionTreeMenuItems(ctx), ctx);
    expect(ids).toContain('vacuum');
    expect(ids).toContain('integrity-check');
    expect(ids).not.toContain('create-db');
  });
});

describe('databaseMenuItems (database node)', () => {
  it('shows database-scoped ops when connected', () => {
    const ctx: MenuContext = { capabilities: pgCaps, isDbConnected: true };
    expect(visibleIds(databaseMenuItems(ctx), ctx)).toEqual([
      'new-query', 'create-schema', 'refresh', 'export-db', 'restore',
      'disconnect', 'rename-db', 'edit-db', 'drop-db',
    ]);
  });

  it('shows connect + database ops when disconnected', () => {
    const ctx: MenuContext = { capabilities: pgCaps, isDbConnected: false };
    expect(visibleIds(databaseMenuItems(ctx), ctx)).toEqual([
      'connect', 'rename-db', 'edit-db', 'drop-db',
    ]);
  });

  it('differs from the host menu for the same connected engine', () => {
    const hostCtx: MenuContext = { capabilities: pgCaps, isConnected: true, engineType: 'postgres' };
    const dbCtx: MenuContext = { capabilities: pgCaps, isDbConnected: true };
    expect(visibleIds(connectionTreeMenuItems(hostCtx), hostCtx))
      .not.toEqual(visibleIds(databaseMenuItems(dbCtx), dbCtx));
  });
});

describe('schemaMenuItems (schema node)', () => {
  it('shows schema-scoped ops for a SQL engine', () => {
    const ctx: MenuContext = { capabilities: pgCaps };
    expect(visibleIds(schemaMenuItems(ctx), ctx)).toEqual([
      'new-query', 'view-erd', 'refresh', 'export-schema', 'restore',
      'create-schema', 'rename-schema', 'drop-schema',
    ]);
  });

  it('hides restore when the engine cannot restore', () => {
    const ctx: MenuContext = { capabilities: sqliteCaps };
    expect(visibleIds(schemaMenuItems(ctx), ctx)).not.toContain('restore');
  });
});

describe('tableMenuItems (table node)', () => {
  it('includes rename for SQL engines', () => {
    const ctx: MenuContext = { capabilities: pgCaps };
    const ids = visibleIds(tableMenuItems(), ctx);
    expect(ids).toContain('rename');
    expect(ids.indexOf('rename')).toBeGreaterThan(ids.indexOf('duplicate'));
    expect(ids.indexOf('rename')).toBeLessThan(ids.indexOf('truncate'));
  });

  it('hides SQL-dependent items for non-SQL engines', () => {
    const ctx: MenuContext = { capabilities: { sql: false } as unknown as EngineCapabilities };
    const ids = visibleIds(tableMenuItems(), ctx);
    expect(ids).not.toContain('rename');
    expect(ids).not.toContain('drop');
  });
});
