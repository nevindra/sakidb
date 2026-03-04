import { invoke } from '@tauri-apps/api/core';
import { getRuntimeId } from './connections.svelte';

export async function exportTableCsv(
  savedConnectionId: string,
  databaseName: string,
  schema: string,
  table: string,
  filePath: string,
  whereClause?: string,
  includeHeader: boolean = true,
): Promise<number> {
  const rid = getRuntimeId(savedConnectionId, databaseName);
  if (!rid) throw new Error('Not connected');
  return await invoke('export_table_csv', {
    activeConnectionId: rid,
    schema,
    table,
    filePath,
    whereClause: whereClause ?? null,
    includeHeader,
  });
}

export async function restoreFromSql(
  savedConnectionId: string,
  databaseName: string,
  filePath: string,
  schema?: string,
  continueOnError: boolean = false,
): Promise<void> {
  const rid = getRuntimeId(savedConnectionId, databaseName);
  if (!rid) throw new Error('Not connected');
  await invoke('restore_from_sql', {
    activeConnectionId: rid,
    filePath,
    schema: schema ?? null,
    continueOnError,
  });
}

export async function cancelRestore(): Promise<void> {
  await invoke('cancel_restore');
}

export async function cancelExport(savedConnectionId: string, databaseName: string): Promise<void> {
  const rid = getRuntimeId(savedConnectionId, databaseName);
  if (!rid) return;
  await invoke('cancel_export', { activeConnectionId: rid });
}

export async function exportTableSql(
  savedConnectionId: string,
  databaseName: string,
  schema: string,
  table: string,
  filePath: string,
  includeDdl: boolean = true,
  includeData: boolean = true,
): Promise<number> {
  const rid = getRuntimeId(savedConnectionId, databaseName);
  if (!rid) throw new Error('Not connected');
  return await invoke('export_table_sql', {
    activeConnectionId: rid,
    schema,
    table,
    filePath,
    includeDdl,
    includeData,
  });
}
