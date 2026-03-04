import type { CompletionContext, CompletionResult, Completion } from '@codemirror/autocomplete';
import type { CompletionBundle, CompletionColumn } from '$lib/types';

/**
 * SQL keywords that trigger table completions when they appear before the cursor.
 * These are positions where the user likely wants to type a table name.
 */
const TABLE_CONTEXT_KEYWORDS = /\b(FROM|JOIN|INNER\s+JOIN|LEFT\s+JOIN|RIGHT\s+JOIN|FULL\s+JOIN|CROSS\s+JOIN|INTO|UPDATE|DELETE\s+FROM|TABLE|REFERENCES)\s+$/i;

/**
 * SQL keywords that trigger column or table completions (e.g., SELECT, WHERE, ORDER BY).
 */
const COLUMN_CONTEXT_KEYWORDS = /\b(SELECT|WHERE|AND|OR|ON|SET|GROUP\s+BY|ORDER\s+BY|HAVING|WHEN|THEN|ELSE|CASE|VALUES|RETURNING|DISTINCT|BETWEEN|IN|LIKE|ILIKE|NOT)\s+$/i;

/**
 * Matches "tablename." to trigger column completion for that table.
 */
const DOT_ACCESS_PATTERN = /\b([\w]+)\.\s*$/;

/**
 * Common SQL keywords for autocompletion, grouped by typical usage position.
 */
const SQL_KEYWORDS = [
  // Clauses (high priority after SELECT ... or table references)
  'SELECT', 'FROM', 'WHERE', 'JOIN', 'INNER JOIN', 'LEFT JOIN', 'RIGHT JOIN',
  'FULL JOIN', 'CROSS JOIN', 'ON', 'AND', 'OR', 'NOT', 'IN', 'EXISTS',
  'BETWEEN', 'LIKE', 'ILIKE', 'IS', 'NULL', 'AS',
  'ORDER BY', 'GROUP BY', 'HAVING', 'LIMIT', 'OFFSET', 'FETCH',
  'UNION', 'UNION ALL', 'INTERSECT', 'EXCEPT',
  'INSERT INTO', 'VALUES', 'UPDATE', 'SET', 'DELETE FROM',
  'CREATE TABLE', 'ALTER TABLE', 'DROP TABLE',
  'CREATE INDEX', 'DROP INDEX',
  'DISTINCT', 'ALL', 'CASE', 'WHEN', 'THEN', 'ELSE', 'END',
  'ASC', 'DESC', 'NULLS FIRST', 'NULLS LAST',
  'TRUE', 'FALSE', 'DEFAULT',
  'RETURNING', 'WITH', 'RECURSIVE',
  'LATERAL', 'USING', 'NATURAL',
  'GRANT', 'REVOKE', 'BEGIN', 'COMMIT', 'ROLLBACK',
  'EXPLAIN', 'EXPLAIN ANALYZE', 'VACUUM', 'ANALYZE',
  'COALESCE', 'CAST', 'FILTER', 'OVER', 'PARTITION BY', 'WINDOW',
  'COUNT', 'SUM', 'AVG', 'MIN', 'MAX',
  'ROW_NUMBER', 'RANK', 'DENSE_RANK', 'LAG', 'LEAD',
];

/**
 * Extracts table names and aliases referenced in FROM/JOIN clauses from the query text.
 * Returns a map of alias/name → real table name (lowercased).
 *
 * Handles: FROM t, FROM t alias, FROM t AS alias, JOIN t ON ..., FROM t1, t2
 */
function extractReferencedTables(
  queryText: string,
  knownTables: Set<string>,
): Map<string, string> {
  const refs = new Map<string, string>();
  // Match table references after FROM/JOIN keywords
  // Captures: FROM/JOIN table [AS] [alias] [,table [AS] [alias]]*
  const clausePattern = /\b(?:FROM|JOIN|INNER\s+JOIN|LEFT\s+JOIN|RIGHT\s+JOIN|FULL\s+JOIN|CROSS\s+JOIN|UPDATE|INTO)\s+/gi;

  // SQL keywords that end a table-list context
  const stopKeywords = new Set([
    'where', 'set', 'on', 'join', 'inner', 'left', 'right', 'full', 'cross',
    'order', 'group', 'having', 'limit', 'offset', 'union', 'intersect',
    'except', 'returning', 'values', 'select', 'window', 'fetch',
  ]);

  let match;
  while ((match = clausePattern.exec(queryText)) !== null) {
    const rest = queryText.slice(match.index + match[0].length);
    // Parse comma-separated table references until a stop keyword or clause end
    const tokens = rest.split(/\s+/);
    let i = 0;
    while (i < tokens.length) {
      // Clean trailing commas/parens
      let token = tokens[i].replace(/[(),;]+$/, '').replace(/^[(),;]+/, '');
      if (!token || stopKeywords.has(token.toLowerCase())) break;
      const tableName = token.toLowerCase();
      if (!knownTables.has(tableName)) { i++; continue; }

      // Check for alias: FROM table AS alias, or FROM table alias
      const next = tokens[i + 1]?.replace(/[(),;]+$/, '').toLowerCase();
      if (next === 'as' && tokens[i + 2]) {
        const alias = tokens[i + 2].replace(/[(),;]+$/, '').toLowerCase();
        refs.set(alias, tableName);
        refs.set(tableName, tableName);
        i += 3;
      } else if (next && !stopKeywords.has(next) && !knownTables.has(next) && next !== ','
                 && !/^(on|where|join|left|right|inner|full|cross|order|group|having|limit|set)$/i.test(next)) {
        refs.set(next, tableName);
        refs.set(tableName, tableName);
        i += 2;
      } else {
        refs.set(tableName, tableName);
        i++;
      }

      // Skip comma separator
      if (tokens[i] === ',') i++;
    }
  }
  return refs;
}

/**
 * Creates a custom CodeMirror completion source for schema-aware SQL autocompletion.
 *
 * @param getBundle - Function to get the cached completion bundle (tables + functions)
 * @param getColumns - Function to lazily load columns for a given table
 */
export function createSqlCompletionSource(
  getBundle: () => CompletionBundle | null,
  getColumns: (tableName: string) => Promise<CompletionColumn[]>,
): (ctx: CompletionContext) => Promise<CompletionResult | null> {
  return async (ctx: CompletionContext): Promise<CompletionResult | null> => {
    const bundle = getBundle();
    if (!bundle) return null;

    const knownTableNames = new Set(bundle.tables.map(t => t.name.toLowerCase()));

    // Get full document text up to cursor for query-level analysis
    const fullTextBefore = ctx.state.sliceDoc(0, ctx.pos);

    // Get text before cursor on current line for context analysis
    const line = ctx.state.doc.lineAt(ctx.pos);
    const textBefore = ctx.state.sliceDoc(line.from, ctx.pos);

    // Extract tables referenced in FROM/JOIN clauses (alias → real table name)
    const referencedTables = extractReferencedTables(fullTextBefore, knownTableNames);

    // Check for "name." dot access → column completion (supports aliases)
    const dotMatch = textBefore.match(DOT_ACCESS_PATTERN);
    if (dotMatch) {
      const name = dotMatch[1].toLowerCase();
      // Resolve alias to real table name, or check if it's a direct table name
      const realTable = referencedTables.get(name)
        ?? (knownTableNames.has(name) ? name : null);
      if (realTable) {
        // Find the original-cased table name for the API call
        const tableEntry = bundle.tables.find(t => t.name.toLowerCase() === realTable);
        if (tableEntry) {
          const columns = await getColumns(tableEntry.name);
          if (columns.length === 0) return null;
          const options: Completion[] = columns.map(col => ({
            label: col.name,
            detail: formatColumnDetail(col),
            type: 'property',
            boost: col.is_primary_key ? 2 : 0,
          }));
          return {
            from: ctx.pos,
            options,
            validFor: /^\w*$/,
          };
        }
      }
    }

    // Get the word being typed
    const word = ctx.matchBefore(/\w+/);
    if (!word && !ctx.explicit) return null;

    const from = word?.from ?? ctx.pos;
    const typed = word?.text.toLowerCase() ?? '';

    // Determine context from text before the current word
    const beforeWord = ctx.state.sliceDoc(
      Math.max(0, line.from),
      from
    );

    const isTableContext = TABLE_CONTEXT_KEYWORDS.test(beforeWord);
    const isColumnContext = COLUMN_CONTEXT_KEYWORDS.test(beforeWord);

    const options: Completion[] = [];

    // Add SQL keyword completions (high boost so they rank above schema objects)
    for (const kw of SQL_KEYWORDS) {
      const kwLower = kw.toLowerCase();
      const firstWord = kwLower.split(' ')[0];
      if (typed && !firstWord.startsWith(typed)) continue;
      // Don't suggest keywords the user just typed as context
      const lastKeyword = beforeWord.trim().split(/\s+/).pop()?.toUpperCase();
      if (lastKeyword === kw.split(' ')[0]) continue;
      options.push({
        label: kw,
        type: 'keyword',
        boost: isTableContext ? -5 : 20,
      });
    }

    // Always suggest tables (they're useful in most contexts)
    for (const table of bundle.tables) {
      if (typed && !table.name.toLowerCase().startsWith(typed)) continue;
      options.push({
        label: table.name,
        detail: table.kind,
        type: table.kind === 'view' ? 'class' : table.kind === 'materialized_view' ? 'class' : 'type',
        boost: isTableContext ? 10 : 0,
      });
    }

    // Add functions in non-table-specific contexts
    if (!isTableContext) {
      for (const fn of bundle.functions) {
        if (typed && !fn.name.toLowerCase().startsWith(typed)) continue;
        options.push({
          label: fn.argument_types ? `${fn.name}(${fn.argument_types})` : `${fn.name}()`,
          displayLabel: fn.name,
          detail: fn.return_type ? `→ ${fn.return_type}` : fn.kind,
          type: 'function',
          apply: fn.argument_types ? `${fn.name}(` : `${fn.name}()`,
        });
      }
    }

    // In column contexts, suggest columns scoped to referenced tables
    if (isColumnContext && !isTableContext) {
      const hasReferencedTables = referencedTables.size > 0;

      if (hasReferencedTables) {
        // Only suggest columns from tables in FROM/JOIN clauses
        const seenTables = new Set<string>();
        for (const realTable of referencedTables.values()) {
          if (seenTables.has(realTable)) continue;
          seenTables.add(realTable);
          const tableEntry = bundle.tables.find(t => t.name.toLowerCase() === realTable);
          if (!tableEntry) continue;
          const columns = await getColumns(tableEntry.name);
          for (const col of columns) {
            if (typed && !col.name.toLowerCase().startsWith(typed)) continue;
            options.push({
              label: col.name,
              detail: `${tableEntry.name}.${col.data_type}`,
              type: 'property',
              boost: col.is_primary_key ? 5 : 2,
            });
          }
        }
      }
      // If no FROM/JOIN tables found yet, don't suggest columns from every table
    }

    if (options.length === 0) return null;

    return {
      from,
      options,
      validFor: /^\w*$/,
    };
  };
}

function formatColumnDetail(col: CompletionColumn): string {
  let detail = col.data_type;
  if (col.is_primary_key) detail += ', PK';
  if (col.is_nullable) detail += ', null';
  return detail;
}
