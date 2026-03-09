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
    ///
    /// Uses `&str` slice copies instead of per-byte `ch as char` to correctly
    /// preserve multi-byte UTF-8 sequences. All SQL syntax delimiters are ASCII,
    /// so we only need byte-level checks for state transitions.
    pub fn feed(&mut self, chunk: &str) -> Vec<String> {
        let mut stmts = Vec::new();
        let bytes = chunk.as_bytes();
        let len = bytes.len();
        let mut i = 0;
        // Start of a run of bytes to bulk-copy into buf.
        let mut run_start = 0;
        let mut in_run = false;

        // Flush buffered run of bytes as a &str slice (preserves UTF-8).
        macro_rules! flush_run {
            () => {
                if in_run {
                    self.buf.push_str(&chunk[run_start..i]);
                    #[allow(unused_assignments)]
                    {
                        in_run = false;
                    }
                }
            };
        }

        macro_rules! start_run {
            () => {
                if !in_run {
                    run_start = i;
                    in_run = true;
                }
            };
        }

        while i < len {
            let ch = bytes[i];

            match self.state {
                ParserState::Normal => {
                    // Line comment start
                    if ch == b'-' && i + 1 < len && bytes[i + 1] == b'-' {
                        flush_run!();
                        self.buf.push_str("--");
                        i += 2;
                        self.state = ParserState::LineComment;
                        continue;
                    }
                    // Edge: `-` at end of chunk — buffer it, stay Normal
                    if ch == b'-' && i + 1 == len {
                        flush_run!();
                        self.buf.push('-');
                        i += 1;
                        continue;
                    }

                    // Block comment start
                    if ch == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
                        flush_run!();
                        self.buf.push_str("/*");
                        i += 2;
                        self.state = ParserState::BlockComment { depth: 1 };
                        continue;
                    }

                    // Single-quoted string
                    if ch == b'\'' {
                        flush_run!();
                        self.buf.push('\'');
                        i += 1;
                        self.state = ParserState::SingleQuote;
                        continue;
                    }

                    // Double-quoted identifier
                    if ch == b'"' {
                        flush_run!();
                        self.buf.push('"');
                        i += 1;
                        self.state = ParserState::DoubleQuote;
                        continue;
                    }

                    // Dollar-quoting (PG)
                    if self.opts.dollar_quoting && ch == b'$' {
                        flush_run!();
                        self.buf.push('$');
                        self.dollar_tag.clear();
                        self.dollar_tag.push(b'$');
                        i += 1;
                        self.state = ParserState::DollarTag;
                        continue;
                    }

                    // Semicolon — statement boundary
                    if ch == b';' {
                        flush_run!();
                        let trimmed = self.buf.trim();
                        if !trimmed.is_empty() {
                            stmts.push(trimmed.to_string());
                        }
                        self.buf.clear();
                        i += 1;
                        continue;
                    }

                    start_run!();
                    i += 1;
                }

                ParserState::LineComment => {
                    start_run!();
                    i += 1;
                    if ch == b'\n' {
                        flush_run!();
                        self.state = ParserState::Normal;
                    }
                }

                ParserState::BlockComment { depth } => {
                    if ch == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
                        flush_run!();
                        self.buf.push_str("/*");
                        i += 2;
                        self.state = ParserState::BlockComment { depth: depth + 1 };
                    } else if ch == b'*' && i + 1 < len && bytes[i + 1] == b'/' {
                        flush_run!();
                        self.buf.push_str("*/");
                        i += 2;
                        if depth == 1 {
                            self.state = ParserState::Normal;
                        } else {
                            self.state = ParserState::BlockComment { depth: depth - 1 };
                        }
                    } else {
                        start_run!();
                        i += 1;
                    }
                }

                ParserState::SingleQuote => {
                    if ch == b'\'' {
                        if i + 1 < len && bytes[i + 1] == b'\'' {
                            // escaped '' — include both quotes
                            start_run!();
                            i += 2;
                        } else {
                            // closing quote
                            start_run!();
                            i += 1;
                            flush_run!();
                            self.state = ParserState::Normal;
                        }
                    } else {
                        start_run!();
                        i += 1;
                    }
                }

                ParserState::DoubleQuote => {
                    if ch == b'"' {
                        if i + 1 < len && bytes[i + 1] == b'"' {
                            // escaped ""
                            start_run!();
                            i += 2;
                        } else {
                            // closing quote
                            start_run!();
                            i += 1;
                            flush_run!();
                            self.state = ParserState::Normal;
                        }
                    } else {
                        start_run!();
                        i += 1;
                    }
                }

                ParserState::DollarTag => {
                    if ch == b'$' {
                        // Tag is complete: dollar_tag contains `$tag$`
                        flush_run!();
                        self.buf.push('$');
                        self.dollar_tag.push(b'$');
                        let tag_len = self.dollar_tag.len() as u16;
                        i += 1;
                        self.state = ParserState::DollarBody { tag_len };
                    } else if ch.is_ascii_alphanumeric() || ch == b'_' {
                        flush_run!();
                        self.buf.push(ch as char); // safe: ASCII alphanumeric
                        self.dollar_tag.push(ch);
                        i += 1;
                    } else {
                        // Not a valid dollar-quote, treat as normal text
                        start_run!();
                        self.dollar_tag.clear();
                        i += 1;
                        self.state = ParserState::Normal;
                    }
                }

                ParserState::DollarBody { tag_len } => {
                    start_run!();
                    i += 1;
                    let tl = tag_len as usize;
                    // Check if we just completed the closing tag
                    if ch == b'$' {
                        flush_run!();
                        if self.buf.len() >= tl {
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
        }

        // Flush any trailing run
        if in_run {
            self.buf.push_str(&chunk[run_start..len]);
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

