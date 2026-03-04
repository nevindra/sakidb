/**
 * Lightweight SQL statement splitter for cursor-aware execution.
 * Mirrors the Rust-side split_sql_statements logic (which is authoritative for execution).
 * This is only used for finding the statement at cursor position in the editor.
 */

export interface StatementRange {
  start: number;  // char offset in original string
  end: number;    // char offset (exclusive)
  text: string;   // trimmed statement text
}

export function splitStatements(sql: string): StatementRange[] {
  const statements: StatementRange[] = [];
  let currentStart = 0;
  let i = 0;
  const len = sql.length;

  while (i < len) {
    const ch = sql[i];

    // Line comment
    if (ch === '-' && i + 1 < len && sql[i + 1] === '-') {
      i += 2;
      while (i < len && sql[i] !== '\n') i++;
      continue;
    }

    // Block comment
    if (ch === '/' && i + 1 < len && sql[i + 1] === '*') {
      i += 2;
      let depth = 1;
      while (i < len && depth > 0) {
        if (sql[i] === '/' && i + 1 < len && sql[i + 1] === '*') {
          depth++;
          i += 2;
        } else if (sql[i] === '*' && i + 1 < len && sql[i + 1] === '/') {
          depth--;
          i += 2;
        } else {
          i++;
        }
      }
      continue;
    }

    // Single-quoted string
    if (ch === "'") {
      i++;
      while (i < len) {
        if (sql[i] === "'") {
          i++;
          if (i < len && sql[i] === "'") {
            i++;
          } else {
            break;
          }
        } else {
          i++;
        }
      }
      continue;
    }

    // Dollar-quoted string
    if (ch === '$') {
      const start = i;
      i++;
      while (i < len && (/[a-zA-Z0-9_]/).test(sql[i])) i++;
      if (i < len && sql[i] === '$') {
        const tag = sql.slice(start, i + 1);
        i++;
        const closeIdx = sql.indexOf(tag, i);
        if (closeIdx !== -1) {
          i = closeIdx + tag.length;
        } else {
          i = len;
        }
      }
      continue;
    }

    // Semicolon — statement boundary
    if (ch === ';') {
      const text = sql.slice(currentStart, i).trim();
      if (text.length > 0) {
        statements.push({ start: currentStart, end: i, text });
      }
      i++;
      currentStart = i;
      continue;
    }

    i++;
  }

  // Last statement
  const text = sql.slice(currentStart).trim();
  if (text.length > 0) {
    statements.push({ start: currentStart, end: len, text });
  }

  return statements;
}

/**
 * Find the statement at the given cursor position.
 * Returns the statement text, or the full SQL if no match found.
 */
export function getStatementAtCursor(sql: string, cursorPos: number): string {
  const stmts = splitStatements(sql);
  for (const stmt of stmts) {
    if (cursorPos >= stmt.start && cursorPos <= stmt.end) {
      return stmt.text;
    }
  }
  // Fallback: return everything
  return sql.trim();
}
