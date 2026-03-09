import type { CellValue, ColumnInfo } from '$lib/types';
import type { SQLDialect } from '@codemirror/lang-sql';
import type { SqlLanguage } from 'sql-formatter';

// -- Param types for DDL operations --

export interface ColumnDraft {
  name: string;
  type: string;
  nullable: boolean;
  defaultValue?: string;
  primaryKey?: boolean;
  unique?: boolean;
  isArray?: boolean;
  precision?: string;   // e.g. "255" for varchar(255), "10,2" for numeric(10,2)
  check?: string;       // e.g. "age > 0"
  comment?: string;
}

export interface ColumnChanges {
  type?: string;
  nullable?: boolean;
  defaultValue?: string | null;
  rename?: string;
}

export interface IndexDraft {
  name: string;
  columns: string[];
  unique: boolean;
  type: string;
}

export interface ForeignKeyDraft {
  name?: string;
  columns: string[];
  refSchema: string;
  refTable: string;
  refColumns: string[];
  onUpdate: string;
  onDelete: string;
}

export interface TriggerDraft {
  name: string;
  timing: string;
  event: string;
  forEach: string;
  functionSchema: string;
  functionName: string;
  condition?: string;
}

export interface PartitionDraft {
  name: string;
  forValues: string;
}

// -- Dialect interface --

export interface SqlDialect {
  // Identifiers
  quoteIdent(name: string): string;
  qualifiedTable(schema: string, table: string): string;

  // Table operations
  dropTable(schema: string, table: string): string;
  truncateTable(schema: string, table: string): string;
  duplicateTable(schema: string, src: string, dst: string, mode: 'structure' | 'data'): string;
  refreshMaterializedView(schema: string, view: string): string | null;

  // Cell literals (for UPDATE/INSERT/DELETE)
  cellLiteral(cell: CellValue, dataType?: string): string;

  // DDL generation
  addColumn(schema: string, table: string, col: ColumnDraft): string;
  alterColumn(schema: string, table: string, colName: string, changes: ColumnChanges): string;
  dropColumn(schema: string, table: string, colName: string): string;
  createIndex(schema: string, table: string, idx: IndexDraft): string;
  dropIndex(schema: string, indexName: string): string;
  addForeignKey(schema: string, table: string, fk: ForeignKeyDraft): string;
  dropConstraint(schema: string, table: string, constraintName: string): string;
  createTrigger(schema: string, table: string, trig: TriggerDraft): string | null;
  dropTrigger(schema: string, table: string, triggerName: string): string;
  toggleTrigger(schema: string, table: string, triggerName: string, enable: boolean): string | null;
  addPartition(schema: string, parentTable: string, part: PartitionDraft): string | null;
  detachPartition(schema: string, parentTable: string, partitionName: string): string | null;

  // Editor integration
  codemirrorDialect(): SQLDialect;
  formatterLanguage(): SqlLanguage;
  explainAnalyzeQuery(sql: string, json: boolean): string | null;

  // Profiling
  statsQuery(schema: string, table: string, col: ColumnInfo): string | null;
  histogramQuery(schema: string, table: string, col: ColumnInfo): string | null;
  uniqueCountQuery(schema: string, table: string, col: ColumnInfo): string | null;
  bulkStatsQuery(schema: string, table: string, columns: ColumnInfo[]): string | null;
  bulkHistogramQuery(schema: string, table: string, columns: ColumnInfo[]): string | null;
  bulkUniqueCountQuery(schema: string, table: string, columns: ColumnInfo[]): string | null;
}
