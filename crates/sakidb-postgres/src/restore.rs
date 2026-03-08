use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use bytes::Bytes;
use deadpool_postgres::Pool;
use futures_util::SinkExt;
use tokio::io::AsyncBufReadExt;
use tracing::{info, warn};

use sakidb_core::types::RestoreProgress;
use sakidb_core::SakiError;

use crate::executor::{format_pg_error, format_pool_error};

/// Maximum number of error messages retained during restore.
/// Beyond this, errors are still counted but messages are dropped.
const MAX_ERROR_MESSAGES: usize = 1000;

pub async fn restore_from_sql(
    pool: &Pool,
    file_path: &str,
    schema: Option<&str>,
    continue_on_error: bool,
    cancelled: &AtomicBool,
    on_progress: Box<dyn Fn(RestoreProgress) + Send + Sync>,
) -> Result<RestoreProgress, SakiError> {
    info!(file_path, continue_on_error, "starting SQL restore");

    let metadata = tokio::fs::metadata(file_path)
        .await
        .map_err(|e| SakiError::QueryFailed(format!("Cannot read file: {e}")))?;
    let total_bytes = metadata.len();

    info!(total_bytes, "restore file opened");

    let file = tokio::fs::File::open(file_path)
        .await
        .map_err(|e| SakiError::QueryFailed(format!("Cannot open file: {e}")))?;
    let mut reader = tokio::io::BufReader::new(file);

    let client = pool
        .get()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;

    // Set search_path for schema-level restore
    if let Some(s) = schema {
        let quoted = s.replace('"', "\"\"");
        client
            .batch_execute(&format!("SET search_path TO \"{quoted}\", public"))
            .await
            .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;
    }

    let start = Instant::now();
    let mut progress = RestoreProgress {
        bytes_read: 0,
        total_bytes,
        statements_executed: 0,
        errors_skipped: 0,
        phase: "Starting".to_string(),
        elapsed_ms: 0,
        error: None,
        error_messages: Vec::new(),
    };

    let mut parser = SqlParser::new();
    let mut batch: Vec<String> = Vec::new();
    let mut line = String::new();
    let mut last_progress = Instant::now();

    const BATCH_SIZE: usize = 100;

    loop {
        line.clear();
        let n = reader
            .read_line(&mut line)
            .await
            .map_err(|e| SakiError::QueryFailed(format!("Read error: {e}")))?;
        if n == 0 {
            break;
        }

        progress.bytes_read += n as u64;

        if cancelled.load(Ordering::Relaxed) {
            info!(
                statements = progress.statements_executed,
                errors = progress.errors_skipped,
                "restore cancelled"
            );
            progress.phase = "Cancelled".to_string();
            progress.elapsed_ms = start.elapsed().as_millis() as u64;
            on_progress(progress.clone());
            return Err(SakiError::Cancelled);
        }

        let stmts = parser.feed_line(&line);

        for stmt in stmts {
            if is_copy_from_stdin(&stmt) {
                // Flush pending batch first
                if !batch.is_empty() {
                    flush_batch(&client, &mut batch, continue_on_error, &mut progress).await?;
                }

                progress.phase = "COPY".to_string();

                // Use a fresh connection for each COPY so that a failed
                // COPY (which can poison the connection state) doesn't
                // break subsequent COPY operations.
                let copy_client = pool
                    .get()
                    .await
                    .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;

                // Apply search_path to the COPY connection too
                if let Some(s) = schema {
                    let quoted = s.replace('"', "\"\"");
                    let _ = copy_client
                        .batch_execute(&format!("SET search_path TO \"{quoted}\", public"))
                        .await;
                }

                let copy_result = copy_client.copy_in(stmt.as_str()).await;

                let copy_err_msg: Option<String> = match copy_result {
                    Ok(sink) => {
                        let mut sink = Box::pin(sink);

                        // Read data lines until \. terminator
                        let mut copy_line = String::new();
                        let mut send_err: Option<String> = None;
                        loop {
                            copy_line.clear();
                            let cn = reader
                                .read_line(&mut copy_line)
                                .await
                                .map_err(|e| SakiError::QueryFailed(format!("Read error: {e}")))?;
                            if cn == 0 {
                                break;
                            }
                            progress.bytes_read += cn as u64;

                            if cancelled.load(Ordering::Relaxed) {
                                progress.phase = "Cancelled".to_string();
                                progress.elapsed_ms = start.elapsed().as_millis() as u64;
                                on_progress(progress.clone());
                                return Err(SakiError::Cancelled);
                            }

                            let trimmed = copy_line
                                .trim_end_matches('\n')
                                .trim_end_matches('\r');
                            if trimmed == "\\." {
                                break;
                            }

                            if send_err.is_none() {
                                if let Err(e) = sink.send(Bytes::copy_from_slice(copy_line.as_bytes())).await {
                                    send_err = Some(format_pg_error(&e));
                                }
                            }

                            // Progress update during COPY
                            if last_progress.elapsed().as_millis() > 100 {
                                progress.elapsed_ms = start.elapsed().as_millis() as u64;
                                on_progress(progress.clone());
                                last_progress = Instant::now();
                            }
                        }

                        if let Some(err) = send_err {
                            Some(err)
                        } else {
                            match sink.as_mut().finish().await {
                                Ok(_) => None,
                                Err(e) => Some(format_pg_error(&e)),
                            }
                        }
                    }
                    Err(e) => {
                        // COPY command failed — drain data lines until \.
                        // so the parser doesn't treat them as SQL
                        let mut drain_line = String::new();
                        loop {
                            drain_line.clear();
                            let dn = reader
                                .read_line(&mut drain_line)
                                .await
                                .map_err(|e| SakiError::QueryFailed(format!("Read error: {e}")))?;
                            if dn == 0 {
                                break;
                            }
                            progress.bytes_read += dn as u64;
                            let trimmed = drain_line
                                .trim_end_matches('\n')
                                .trim_end_matches('\r');
                            if trimmed == "\\." {
                                break;
                            }
                        }
                        Some(format_pg_error(&e))
                    }
                };

                if let Some(err) = copy_err_msg {
                    // Only allocate the truncated label when we actually have an error
                    let copy_label: String = stmt.chars().take(80).collect();
                    if continue_on_error {
                        progress.errors_skipped += 1;
                        if progress.error_messages.len() < MAX_ERROR_MESSAGES {
                            progress.error_messages.push(format!("{copy_label}... → {err}"));
                        }
                    } else {
                        return Err(SakiError::QueryFailed(format!("{copy_label}: {err}")));
                    }
                } else {
                    progress.statements_executed += 1;
                }
            } else {
                progress.phase = "Executing".to_string();
                batch.push(stmt);
                if batch.len() >= BATCH_SIZE {
                    flush_batch(&client, &mut batch, continue_on_error, &mut progress).await?;
                }
            }
        }

        // Periodic progress
        if last_progress.elapsed().as_millis() > 100 {
            progress.elapsed_ms = start.elapsed().as_millis() as u64;
            on_progress(progress.clone());
            last_progress = Instant::now();
        }
    }

    // Flush remaining batch
    if !batch.is_empty() {
        flush_batch(&client, &mut batch, continue_on_error, &mut progress).await?;
    }

    progress.phase = "Complete".to_string();
    progress.elapsed_ms = start.elapsed().as_millis() as u64;
    on_progress(progress.clone());

    info!(
        statements = progress.statements_executed,
        errors = progress.errors_skipped,
        elapsed_ms = progress.elapsed_ms,
        "restore complete"
    );

    Ok(progress)
}

/// Execute a batch of statements, then clear the batch vec.
/// When `continue_on_error` is true and the batch fails, falls back to
/// executing each statement individually so only the broken ones are skipped.
async fn flush_batch(
    client: &deadpool_postgres::Object,
    batch: &mut Vec<String>,
    continue_on_error: bool,
    progress: &mut RestoreProgress,
) -> Result<(), SakiError> {
    if batch.is_empty() {
        return Ok(());
    }

    let sql = batch.join("\n");
    match client.batch_execute(&sql).await {
        Ok(()) => {
            progress.statements_executed += batch.len() as u64;
        }
        Err(e) => {
            if !continue_on_error {
                batch.clear();
                return Err(SakiError::QueryFailed(format_pg_error(&e)));
            }
            warn!(batch_size = batch.len(), "batch failed, retrying one-by-one");
            // Retry one-by-one to skip only the broken statements
            for stmt in batch.iter() {
                match client.batch_execute(stmt).await {
                    Ok(()) => progress.statements_executed += 1,
                    Err(e) => {
                        progress.errors_skipped += 1;
                        if progress.error_messages.len() < MAX_ERROR_MESSAGES {
                            let label: String = stmt.chars().take(80).collect();
                            progress.error_messages.push(format!("{label}... → {}", format_pg_error(&e)));
                        }
                    }
                }
            }
        }
    }

    batch.clear();
    Ok(())
}

/// Check if a SQL statement is a COPY ... FROM STDIN command.
/// Uses zero-allocation ASCII case-insensitive sliding window.
pub(crate) fn is_copy_from_stdin(stmt: &str) -> bool {
    let bytes = stmt.as_bytes();
    if bytes.len() < 5 {
        return false;
    }
    // Check "COPY " prefix case-insensitively
    if !bytes[..5].eq_ignore_ascii_case(b"COPY ") {
        return false;
    }
    // Sliding window search for "FROM STDIN" without allocation
    let needle = b"FROM STDIN";
    let hay = &bytes[5..];
    hay.windows(needle.len()).any(|w| w.eq_ignore_ascii_case(needle))
}

// ── SQL Statement Parser ──

pub(crate) struct SqlParser {
    pub(crate) buf: String,
    pub(crate) in_single_quote: bool,
    pub(crate) in_double_quote: bool,
    pub(crate) dollar_quote_tag: Option<String>,
    pub(crate) block_comment_depth: i32,
}

impl SqlParser {
    pub(crate) fn new() -> Self {
        Self {
            buf: String::new(),
            in_single_quote: false,
            in_double_quote: false,
            dollar_quote_tag: None,
            block_comment_depth: 0,
        }
    }

    /// Parse a line of SQL and return any complete statements found.
    ///
    /// Uses byte-level scanning instead of `Vec<char>` to avoid a heap
    /// allocation on every line. All SQL syntax delimiters are ASCII, and
    /// UTF-8 guarantees no continuation byte matches an ASCII byte.
    pub(crate) fn feed_line(&mut self, line: &str) -> Vec<String> {
        let mut stmts = Vec::new();

        // Skip psql meta-commands (lines starting with \) when not inside
        // a string or comment.
        let in_normal = !self.in_single_quote
            && !self.in_double_quote
            && self.dollar_quote_tag.is_none()
            && self.block_comment_depth == 0;
        if in_normal && line.starts_with('\\') {
            return stmts;
        }

        let bytes = line.as_bytes();
        let len = bytes.len();
        let mut i = 0;
        // Track start of a run of characters to batch-copy via &line[..] slices
        let mut run_start = 0;
        let mut in_run = false;

        macro_rules! flush_run {
            () => {
                if in_run {
                    self.buf.push_str(&line[run_start..i]);
                    #[allow(unused_assignments)]
                    { in_run = false; }
                }
            };
        }

        while i < len {
            let c = bytes[i];
            let next = if i + 1 < len { Some(bytes[i + 1]) } else { None };

            // Inside block comment
            if self.block_comment_depth > 0 {
                if c == b'*' && next == Some(b'/') {
                    self.block_comment_depth -= 1;
                    i += 2;
                } else if c == b'/' && next == Some(b'*') {
                    self.block_comment_depth += 1;
                    i += 2;
                } else {
                    i += 1;
                }
                continue;
            }

            // Inside single quote
            if self.in_single_quote {
                if c == b'\'' {
                    if next == Some(b'\'') {
                        // '' escape — push both and advance
                        flush_run!();
                        self.buf.push_str("''");
                        i += 2;
                    } else {
                        if !in_run {
                            run_start = i;
                            in_run = true;
                        }
                        i += 1;
                        flush_run!();
                        self.in_single_quote = false;
                    }
                } else {
                    if !in_run {
                        run_start = i;
                        in_run = true;
                    }
                    i += 1;
                }
                continue;
            }

            // Inside double-quoted identifier
            if self.in_double_quote {
                if c == b'"' {
                    if next == Some(b'"') {
                        flush_run!();
                        self.buf.push_str("\"\"");
                        i += 2;
                    } else {
                        if !in_run {
                            run_start = i;
                            in_run = true;
                        }
                        i += 1;
                        flush_run!();
                        self.in_double_quote = false;
                    }
                } else {
                    if !in_run {
                        run_start = i;
                        in_run = true;
                    }
                    i += 1;
                }
                continue;
            }

            // Inside dollar quote — match closing tag via byte comparison
            if self.dollar_quote_tag.is_some() {
                if c == b'$' {
                    let tag = self.dollar_quote_tag.as_ref().unwrap();
                    let end_marker_len = tag.len() + 2; // $ + tag + $
                    if i + end_marker_len <= len {
                        let tag_matches = if tag.is_empty() {
                            bytes[i + 1] == b'$'
                        } else {
                            i + 1 + tag.len() < len
                                && bytes[i + 1 + tag.len()] == b'$'
                                && tag.as_bytes() == &bytes[i + 1..i + 1 + tag.len()]
                        };
                        if tag_matches {
                            // Flush run up to (not including) `$`, then push the full closing marker
                            flush_run!();
                            self.buf.push_str(&line[i..i + end_marker_len]);
                            i += end_marker_len;
                            self.dollar_quote_tag = None;
                            continue;
                        }
                    }
                }
                if !in_run {
                    run_start = i;
                    in_run = true;
                }
                i += 1;
                continue;
            }

            // Normal mode
            match c {
                b'-' if next == Some(b'-') => {
                    // Line comment — skip rest of line
                    flush_run!();
                    break;
                }
                b'/' if next == Some(b'*') => {
                    flush_run!();
                    self.block_comment_depth = 1;
                    i += 2;
                }
                b'\'' => {
                    flush_run!();
                    self.buf.push('\'');
                    self.in_single_quote = true;
                    i += 1;
                }
                b'"' => {
                    flush_run!();
                    self.buf.push('"');
                    self.in_double_quote = true;
                    i += 1;
                }
                b'$' => {
                    flush_run!();
                    if let Some((tag, marker_len)) = extract_dollar_tag_from_bytes(&bytes[i..]) {
                        self.buf.push_str(&line[i..i + marker_len]);
                        self.dollar_quote_tag = Some(tag);
                        i += marker_len;
                    } else {
                        self.buf.push('$');
                        i += 1;
                    }
                }
                b';' => {
                    flush_run!();
                    self.buf.push(';');
                    let stmt = self.buf.trim().to_string();
                    self.buf.clear();
                    if !stmt.is_empty() {
                        stmts.push(stmt);
                    }
                    i += 1;
                }
                _ => {
                    if !in_run {
                        run_start = i;
                        in_run = true;
                    }
                    i += 1;
                }
            }
        }

        // Flush final run
        flush_run!();

        // Preserve newline for multi-line statements
        if !self.buf.trim().is_empty() {
            self.buf.push('\n');
        }

        stmts
    }
}

/// Extract a dollar-quote tag from a byte slice starting with `$`.
/// Returns (tag, total_marker_byte_length) where marker is `$tag$`.
/// Works directly on byte slices — zero allocation for the scan;
/// only allocates a `String` for the tag if one is found.
pub(crate) fn extract_dollar_tag_from_bytes(bytes: &[u8]) -> Option<(String, usize)> {
    if bytes.is_empty() || bytes[0] != b'$' {
        return None;
    }
    // Find closing $
    let end = bytes[1..].iter().position(|&b| b == b'$')?;
    let tag_bytes = &bytes[1..1 + end];

    // Tag must be empty (for $$) or valid ASCII identifier chars
    if tag_bytes.is_empty() {
        return Some((String::new(), 2)); // $$
    }
    let first = tag_bytes[0];
    if !first.is_ascii_alphabetic() && first != b'_' {
        return None;
    }
    if tag_bytes.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'_') {
        // SAFETY: we verified all bytes are ASCII alphanumeric or underscore
        let tag = unsafe { std::str::from_utf8_unchecked(tag_bytes) }.to_string();
        Some((tag, end + 2)) // $tag$
    } else {
        None
    }
}

