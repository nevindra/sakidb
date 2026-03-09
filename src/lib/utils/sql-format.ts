import { format, type SqlLanguage } from 'sql-formatter';

export function formatSql(sql: string, language: SqlLanguage = 'postgresql'): string {
  return format(sql, {
    language,
    tabWidth: 2,
    useTabs: false,
    keywordCase: 'upper',
    dataTypeCase: 'upper',
    functionCase: 'lower',
  });
}
