import type { EngineType } from '$lib/types';
import type { SqlDialect } from './types';
import { postgresDialect } from './postgres';
import { sqliteDialect } from './sqlite';
import { oracleDialect } from './oracle';

export type { SqlDialect } from './types';
export type { ColumnDraft, ColumnChanges, IndexDraft, ForeignKeyDraft, TriggerDraft, PartitionDraft } from './types';

export function getDialect(engine: EngineType): SqlDialect {
  switch (engine) {
    case 'postgres':   return postgresDialect;
    case 'sqlite':     return sqliteDialect;
    case 'oracle':     return oracleDialect;
    case 'duckdb':
    case 'clickhouse':
    case 'redis':
    case 'mongodb':
      throw new Error(`No SQL dialect for ${engine}`);
  }
  const _: never = engine;
  throw new Error(`Unknown engine: ${engine}`);
}
