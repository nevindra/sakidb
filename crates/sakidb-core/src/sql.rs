//! SQL statement splitting utilities shared across all drivers.
//!
//! Two APIs:
//! - `split_sql_statements` — one-shot split for `execute_multi` style calls.
//! - `StreamingSqlSplitter` — incremental parser for streaming large files.

/// Options controlling dialect-specific parsing.
#[derive(Debug, Clone, Default)]
pub struct SqlSplitOptions {
    /// Enable PostgreSQL `$tag$...$tag$` dollar-quoting.
    pub dollar_quoting: bool,
}

// ── One-shot splitter (returns borrowed slices) ──

/// Split SQL text into individual statements on `;` boundaries, respecting
/// string literals, identifiers, and comments. Returns borrowed slices into the
/// input — no allocations per statement.
pub fn split_sql_statements(sql: &str) -> Vec<&str> {
    split_sql_statements_with(sql, &SqlSplitOptions::default())
}

/// Like [`split_sql_statements`] but with explicit dialect options.
pub fn split_sql_statements_with<'a>(sql: &'a str, opts: &SqlSplitOptions) -> Vec<&'a str> {
    let mut statements: Vec<&str> = Vec::new();
    let bytes = sql.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    // Start of the current statement (inclusive, before trimming).
    let mut stmt_start = 0;

    while i < len {
        let ch = bytes[i];

        // Line comment
        if ch == b'-' && i + 1 < len && bytes[i + 1] == b'-' {
            i += 2;
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        // Block comment (nestable)
        if ch == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
            i += 2;
            let mut depth = 1u32;
            while i < len && depth > 0 {
                if bytes[i] == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
                    depth += 1;
                    i += 2;
                } else if bytes[i] == b'*' && i + 1 < len && bytes[i + 1] == b'/' {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            continue;
        }

        // Single-quoted string ('' escaping)
        if ch == b'\'' {
            i += 1;
            while i < len {
                if bytes[i] == b'\'' {
                    i += 1;
                    if i < len && bytes[i] == b'\'' {
                        i += 1; // escaped ''
                    } else {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            continue;
        }

        // Double-quoted identifier ("" escaping)
        if ch == b'"' {
            i += 1;
            while i < len {
                if bytes[i] == b'"' {
                    i += 1;
                    if i < len && bytes[i] == b'"' {
                        i += 1; // escaped ""
                    } else {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            continue;
        }

        // Dollar-quoted string (PostgreSQL)
        if opts.dollar_quoting && ch == b'$' {
            let tag_start = i;
            i += 1;
            while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            if i < len && bytes[i] == b'$' {
                let tag_end = i + 1;
                let tag_len = tag_end - tag_start;
                i = tag_end;
                loop {
                    if i >= len {
                        break;
                    }
                    if bytes[i] == b'$'
                        && i + tag_len <= len
                        && bytes[i..i + tag_len] == bytes[tag_start..tag_end]
                    {
                        i += tag_len;
                        break;
                    }
                    i += 1;
                }
            }
            // If not a dollar-quote (no closing $), we already advanced past
            // the identifier chars — just continue.
            continue;
        }

        // Semicolon — statement boundary
        if ch == b';' {
            let trimmed = sql[stmt_start..i].trim();
            if !trimmed.is_empty() {
                statements.push(trimmed);
            }
            i += 1;
            stmt_start = i;
            continue;
        }

        i += 1;
    }

    // Last statement (no trailing semicolon)
    let trimmed = sql[stmt_start..].trim();
    if !trimmed.is_empty() {
        statements.push(trimmed);
    }

    statements
}

// ── Streaming splitter ──

/// Parser state for the streaming SQL splitter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParserState {
    Normal,
    LineComment,
    BlockComment { depth: u32 },
    SingleQuote,
    DoubleQuote,
    /// Dollar-quoting (PG). `tag_len` is the length of `$tag$` including both `$`.
    DollarBody { tag_len: u16 },
    /// Accumulating the tag portion between `$` chars before the body starts.
    DollarTag,
}

/// Incremental SQL statement splitter for streaming file reads.
///
/// Feed chunks of SQL text via [`feed`] and collect complete statements.
/// Call [`finish`] after the last chunk to flush any trailing statement.
pub struct StreamingSqlSplitter {
    opts: SqlSplitOptions,
    state: ParserState,
    /// Buffer accumulating the current statement.
    buf: String,
    /// When in DollarTag state, the start tag bytes so far (including leading `$`).
    dollar_tag: Vec<u8>,
}

impl StreamingSqlSplitter {
    pub fn new(opts: SqlSplitOptions) -> Self {
        Self {
            opts,
            state: ParserState::Normal,
            buf: String::with_capacity(4096),
            dollar_tag: Vec::new(),
        }
    }

    /// Feed a chunk of SQL text. Returns any complete statements found.
    pub fn feed(&mut self, chunk: &str) -> Vec<String> {
        let mut stmts = Vec::new();
        let bytes = chunk.as_bytes();
        let len = bytes.len();
        let mut i = 0;

        while i < len {
            let ch = bytes[i];

            match self.state {
                ParserState::Normal => {
                    // Line comment start
                    if ch == b'-' && i + 1 < len && bytes[i + 1] == b'-' {
                        self.buf.push('-');
                        self.buf.push('-');
                        i += 2;
                        self.state = ParserState::LineComment;
                        continue;
                    }
                    // Edge: `-` at end of chunk — buffer it, stay Normal
                    if ch == b'-' && i + 1 == len {
                        self.buf.push('-');
                        i += 1;
                        continue;
                    }

                    // Block comment start
                    if ch == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
                        self.buf.push('/');
                        self.buf.push('*');
                        i += 2;
                        self.state = ParserState::BlockComment { depth: 1 };
                        continue;
                    }

                    // Single-quoted string
                    if ch == b'\'' {
                        self.buf.push('\'');
                        i += 1;
                        self.state = ParserState::SingleQuote;
                        continue;
                    }

                    // Double-quoted identifier
                    if ch == b'"' {
                        self.buf.push('"');
                        i += 1;
                        self.state = ParserState::DoubleQuote;
                        continue;
                    }

                    // Dollar-quoting (PG)
                    if self.opts.dollar_quoting && ch == b'$' {
                        self.buf.push('$');
                        self.dollar_tag.clear();
                        self.dollar_tag.push(b'$');
                        i += 1;
                        self.state = ParserState::DollarTag;
                        continue;
                    }

                    // Semicolon — statement boundary
                    if ch == b';' {
                        let trimmed = self.buf.trim();
                        if !trimmed.is_empty() {
                            stmts.push(trimmed.to_string());
                        }
                        self.buf.clear();
                        i += 1;
                        continue;
                    }

                    self.buf.push(ch as char);
                    i += 1;
                }

                ParserState::LineComment => {
                    self.buf.push(ch as char);
                    i += 1;
                    if ch == b'\n' {
                        self.state = ParserState::Normal;
                    }
                }

                ParserState::BlockComment { depth } => {
                    self.buf.push(ch as char);
                    if ch == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
                        self.buf.push('*');
                        i += 2;
                        self.state = ParserState::BlockComment { depth: depth + 1 };
                    } else if ch == b'*' && i + 1 < len && bytes[i + 1] == b'/' {
                        self.buf.push('/');
                        i += 2;
                        if depth == 1 {
                            self.state = ParserState::Normal;
                        } else {
                            self.state = ParserState::BlockComment { depth: depth - 1 };
                        }
                    } else {
                        i += 1;
                    }
                }

                ParserState::SingleQuote => {
                    self.buf.push(ch as char);
                    i += 1;
                    if ch == b'\'' {
                        if i < len && bytes[i] == b'\'' {
                            self.buf.push('\'');
                            i += 1; // escaped ''
                        } else {
                            self.state = ParserState::Normal;
                        }
                    }
                }

                ParserState::DoubleQuote => {
                    self.buf.push(ch as char);
                    i += 1;
                    if ch == b'"' {
                        if i < len && bytes[i] == b'"' {
                            self.buf.push('"');
                            i += 1; // escaped ""
                        } else {
                            self.state = ParserState::Normal;
                        }
                    }
                }

                ParserState::DollarTag => {
                    self.buf.push(ch as char);
                    if ch == b'$' {
                        // Tag is complete: dollar_tag contains `$tag$`
                        self.dollar_tag.push(b'$');
                        let tag_len = self.dollar_tag.len() as u16;
                        i += 1;
                        self.state = ParserState::DollarBody { tag_len };
                    } else if ch.is_ascii_alphanumeric() || ch == b'_' {
                        self.dollar_tag.push(ch);
                        i += 1;
                    } else {
                        // Not a valid dollar-quote, treat as normal text
                        self.dollar_tag.clear();
                        i += 1;
                        self.state = ParserState::Normal;
                    }
                }

                ParserState::DollarBody { tag_len } => {
                    self.buf.push(ch as char);
                    i += 1;
                    let tl = tag_len as usize;
                    // Check if we just completed the closing tag
                    if ch == b'$' && self.buf.len() >= tl {
                        let buf_bytes = self.buf.as_bytes();
                        let candidate = &buf_bytes[buf_bytes.len() - tl..];
                        if candidate == &self.dollar_tag[..] {
                            self.dollar_tag.clear();
                            self.state = ParserState::Normal;
                        }
                    }
                }
            }
        }

        stmts
    }

    /// Flush any remaining buffered statement after the last chunk.
    pub fn finish(&mut self) -> Option<String> {
        let trimmed = self.buf.trim();
        if trimmed.is_empty() {
            None
        } else {
            let s = trimmed.to_string();
            self.buf.clear();
            self.state = ParserState::Normal;
            Some(s)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── One-shot splitter tests ──

    #[test]
    fn test_split_simple() {
        let stmts = split_sql_statements("SELECT 1; SELECT 2;");
        assert_eq!(stmts, vec!["SELECT 1", "SELECT 2"]);
    }

    #[test]
    fn test_split_no_trailing_semi() {
        let stmts = split_sql_statements("SELECT 1; SELECT 2");
        assert_eq!(stmts, vec!["SELECT 1", "SELECT 2"]);
    }

    #[test]
    fn test_split_string_literal() {
        let stmts = split_sql_statements("SELECT 'hello;world'; SELECT 2");
        assert_eq!(stmts, vec!["SELECT 'hello;world'", "SELECT 2"]);
    }

    #[test]
    fn test_split_escaped_quote() {
        let stmts = split_sql_statements("SELECT 'it''s'; SELECT 2");
        assert_eq!(stmts, vec!["SELECT 'it''s'", "SELECT 2"]);
    }

    #[test]
    fn test_split_line_comment() {
        let stmts = split_sql_statements("SELECT 1; -- comment;\nSELECT 2");
        assert_eq!(stmts, vec!["SELECT 1", "-- comment;\nSELECT 2"]);
    }

    #[test]
    fn test_split_block_comment() {
        let stmts = split_sql_statements("SELECT /* ; */ 1; SELECT 2");
        assert_eq!(stmts, vec!["SELECT /* ; */ 1", "SELECT 2"]);
    }

    #[test]
    fn test_split_empty() {
        let stmts = split_sql_statements("   ;  ;  ");
        assert!(stmts.is_empty());
    }

    #[test]
    fn test_split_single() {
        let stmts = split_sql_statements("SELECT 1");
        assert_eq!(stmts, vec!["SELECT 1"]);
    }

    #[test]
    fn test_split_double_quoted_identifier() {
        let stmts = split_sql_statements("SELECT \"col;name\" FROM t; SELECT 2");
        assert_eq!(stmts, vec!["SELECT \"col;name\" FROM t", "SELECT 2"]);
    }

    // ── Dollar-quoting tests ──

    #[test]
    fn test_split_dollar_quoting() {
        let opts = SqlSplitOptions { dollar_quoting: true };
        let stmts = split_sql_statements_with(
            "SELECT $$hello;world$$; SELECT 2",
            &opts,
        );
        assert_eq!(stmts, vec!["SELECT $$hello;world$$", "SELECT 2"]);
    }

    #[test]
    fn test_split_dollar_tagged() {
        let opts = SqlSplitOptions { dollar_quoting: true };
        let stmts = split_sql_statements_with(
            "CREATE FUNCTION f() RETURNS void AS $body$BEGIN; END;$body$; SELECT 1",
            &opts,
        );
        assert_eq!(
            stmts,
            vec![
                "CREATE FUNCTION f() RETURNS void AS $body$BEGIN; END;$body$",
                "SELECT 1"
            ]
        );
    }

    // ── Streaming splitter tests ──

    #[test]
    fn test_streaming_basic() {
        let mut sp = StreamingSqlSplitter::new(SqlSplitOptions::default());
        let s1 = sp.feed("SELECT 1; SELECT 2;");
        assert_eq!(s1, vec!["SELECT 1", "SELECT 2"]);
        assert_eq!(sp.finish(), None);
    }

    #[test]
    fn test_streaming_across_chunks() {
        let mut sp = StreamingSqlSplitter::new(SqlSplitOptions::default());
        let s1 = sp.feed("SELECT ");
        assert!(s1.is_empty());
        let s2 = sp.feed("1; SEL");
        assert_eq!(s2, vec!["SELECT 1"]);
        let s3 = sp.feed("ECT 2");
        assert!(s3.is_empty());
        assert_eq!(sp.finish(), Some("SELECT 2".to_string()));
    }

    #[test]
    fn test_streaming_string_across_chunks() {
        let mut sp = StreamingSqlSplitter::new(SqlSplitOptions::default());
        let s1 = sp.feed("SELECT 'hel");
        assert!(s1.is_empty());
        let s2 = sp.feed("lo;wor");
        assert!(s2.is_empty());
        let s3 = sp.feed("ld'; SELECT 2;");
        assert_eq!(s3, vec!["SELECT 'hello;world'", "SELECT 2"]);
    }

    #[test]
    fn test_streaming_dollar_quoting() {
        let mut sp = StreamingSqlSplitter::new(SqlSplitOptions { dollar_quoting: true });
        let s1 = sp.feed("SELECT $$he");
        assert!(s1.is_empty());
        let s2 = sp.feed("llo;world$$; SELECT 2;");
        assert_eq!(s2, vec!["SELECT $$hello;world$$", "SELECT 2"]);
    }

    #[test]
    fn test_streaming_block_comment_across_chunks() {
        let mut sp = StreamingSqlSplitter::new(SqlSplitOptions::default());
        let s1 = sp.feed("SELECT /* com");
        assert!(s1.is_empty());
        let s2 = sp.feed("ment; */ 1; SELECT 2;");
        assert_eq!(s2, vec!["SELECT /* comment; */ 1", "SELECT 2"]);
    }
}
