import type { CellValue, ColumnDef } from '$lib/types';

export function cellToPlainText(cell: CellValue): string {
  if (cell === 'Null') return 'NULL';
  if ('Bool' in cell) return String(cell.Bool);
  if ('Int' in cell) return String(cell.Int);
  if ('Float' in cell) return String(cell.Float);
  if ('Text' in cell) return cell.Text;
  if ('Json' in cell) return cell.Json;
  if ('Timestamp' in cell) return cell.Timestamp;
  if ('Bytes' in cell) return cell.Bytes.map((b) => b.toString(16).padStart(2, '0')).join(' ');
  return '';
}

export function cellToJsonValue(cell: CellValue): unknown {
  if (cell === 'Null') return null;
  if ('Bool' in cell) return cell.Bool;
  if ('Int' in cell) return cell.Int;
  if ('Float' in cell) return cell.Float;
  if ('Text' in cell) return cell.Text;
  if ('Json' in cell) {
    try {
      return JSON.parse(cell.Json);
    } catch {
      return cell.Json;
    }
  }
  if ('Timestamp' in cell) return cell.Timestamp;
  if ('Bytes' in cell) return cell.Bytes.map((b) => b.toString(16).padStart(2, '0')).join(' ');
  return null;
}

export function rowToJson(row: CellValue[], columns: ColumnDef[]): string {
  const obj: Record<string, unknown> = {};
  for (let i = 0; i < columns.length; i++) {
    obj[columns[i].name] = cellToJsonValue(row[i]);
  }
  return JSON.stringify(obj, null, 2);
}

function escapeCsvField(value: string): string {
  if (value.includes(',') || value.includes('"') || value.includes('\n')) {
    return `"${value.replace(/"/g, '""')}"`;
  }
  return value;
}

export function rowToCsv(row: CellValue[], columns: ColumnDef[]): string {
  return row.map((cell) => escapeCsvField(cellToPlainText(cell))).join(',');
}

export function rowsToCsv(rows: CellValue[][], columns: ColumnDef[]): string {
  const header = columns.map((c) => escapeCsvField(c.name)).join(',');
  const lines = rows.map((row) => rowToCsv(row, columns));
  return [header, ...lines].join('\n');
}

export async function copyToClipboard(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}
