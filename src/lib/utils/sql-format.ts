import { format } from 'sql-formatter';

export function formatSql(sql: string): string {
  return format(sql, {
    language: 'postgresql',
    tabWidth: 2,
    useTabs: false,
    keywordCase: 'upper',
    dataTypeCase: 'upper',
    functionCase: 'lower',
  });
}
