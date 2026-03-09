import { describe, it, expect } from 'vitest';
import { postgresDialect } from '../postgres';
import { sqliteDialect } from '../sqlite';
import { getDialect } from '../index';
import type { CellValue } from '$lib/types';

describe('getDialect', () => {
  it('returns postgresDialect for postgres', () => {
    expect(getDialect('postgres')).toBe(postgresDialect);
  });

  it('returns sqliteDialect for sqlite', () => {
    expect(getDialect('sqlite')).toBe(sqliteDialect);
  });

  it('throws for unsupported engines', () => {
    expect(() => getDialect('redis')).toThrow('No SQL dialect for redis');
  });
});

describe('PostgresDialect', () => {
  const d = postgresDialect;

  it('quotes identifiers', () => {
    expect(d.quoteIdent('users')).toBe('"users"');
    expect(d.quoteIdent('my"table')).toBe('"my""table"');
  });

  it('qualifies tables with schema', () => {
    expect(d.qualifiedTable('public', 'users')).toBe('"public"."users"');
  });

  it('omits schema when empty', () => {
    expect(d.qualifiedTable('', 'users')).toBe('"users"');
  });

  it('generates DROP TABLE with CASCADE', () => {
    expect(d.dropTable('public', 'users')).toBe('DROP TABLE "public"."users" CASCADE;');
  });

  it('generates TRUNCATE TABLE', () => {
    expect(d.truncateTable('public', 'users')).toBe('TRUNCATE TABLE "public"."users";');
  });

  it('generates duplicate table (structure)', () => {
    expect(d.duplicateTable('public', 'users', 'users_copy', 'structure'))
      .toBe('CREATE TABLE "public"."users_copy" (LIKE "public"."users" INCLUDING ALL);');
  });

  it('generates duplicate table (data)', () => {
    expect(d.duplicateTable('public', 'users', 'users_copy', 'data'))
      .toBe('CREATE TABLE "public"."users_copy" AS SELECT * FROM "public"."users";');
  });

  it('formats cell literal with PG casts', () => {
    const json: CellValue = { Json: '{"a":1}' };
    expect(d.cellLiteral(json)).toBe("'{\"a\":1}'::jsonb");

    const ts: CellValue = { Timestamp: '2024-01-01' };
    expect(d.cellLiteral(ts)).toBe("'2024-01-01'::timestamp");

    const bytes: CellValue = { Bytes: [0xca, 0xfe] };
    expect(d.cellLiteral(bytes)).toBe("'\\xcafe'::bytea");
  });

  it('creates index with USING', () => {
    expect(d.createIndex('public', 'users', { name: 'idx_name', columns: ['name'], unique: false, type: 'btree' }))
      .toBe('CREATE INDEX "idx_name" ON "public"."users" USING btree ("name");');
  });

  it('returns non-null for profiling queries', () => {
    const col = { name: 'age', data_type: 'integer', is_nullable: true, is_primary_key: false, default_value: null };
    expect(d.statsQuery('public', 'users', col)).toContain('PERCENTILE_CONT');
  });
});

describe('SqliteDialect', () => {
  const d = sqliteDialect;

  it('generates DROP TABLE without CASCADE', () => {
    expect(d.dropTable('', 'users')).toBe('DROP TABLE "users";');
  });

  it('generates DELETE FROM instead of TRUNCATE', () => {
    expect(d.truncateTable('', 'users')).toBe('DELETE FROM "users";');
  });

  it('generates duplicate table (structure) with WHERE 0', () => {
    expect(d.duplicateTable('', 'users', 'users_copy', 'structure'))
      .toBe('CREATE TABLE "users_copy" AS SELECT * FROM "users" WHERE 0;');
  });

  it('formats cell literal without PG casts', () => {
    const json: CellValue = { Json: '{"a":1}' };
    expect(d.cellLiteral(json)).toBe("'{\"a\":1}'");

    const ts: CellValue = { Timestamp: '2024-01-01' };
    expect(d.cellLiteral(ts)).toBe("'2024-01-01'");

    const bytes: CellValue = { Bytes: [0xca, 0xfe] };
    expect(d.cellLiteral(bytes)).toBe("X'cafe'");

    const bool: CellValue = { Bool: true };
    expect(d.cellLiteral(bool)).toBe('1');
  });

  it('creates index without USING', () => {
    expect(d.createIndex('', 'users', { name: 'idx_name', columns: ['name'], unique: false, type: 'btree' }))
      .toBe('CREATE INDEX "idx_name" ON "users" ("name");');
  });

  it('returns null for unsupported features', () => {
    const col = { name: 'age', data_type: 'integer', is_nullable: true, is_primary_key: false, default_value: null };
    expect(d.statsQuery('', 'users', col)).toBeNull();
    expect(d.createTrigger('', 'users', { name: 't', timing: 'BEFORE', event: 'INSERT', forEach: 'ROW', functionSchema: '', functionName: '' })).toBeNull();
    expect(d.toggleTrigger('', 'users', 't', true)).toBeNull();
    expect(d.addPartition('', 'users', { name: 'p', forValues: '' })).toBeNull();
  });
});
