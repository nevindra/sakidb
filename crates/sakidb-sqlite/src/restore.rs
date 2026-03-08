use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use rusqlite::Connection;
use tracing::{info, warn};

use sakidb_core::sql::{SqlSplitOptions, StreamingSqlSplitter};
use sakidb_core::types::RestoreProgress;
use sakidb_core::SakiError;

/// Maximum number of error messages retained during restore.
const MAX_ERROR_MESSAGES: usize = 1000;
/// Number of statements per batch.
const BATCH_SIZE: usize = 100;
/// Read buffer size (64 KB).
const READ_BUF_SIZE: usize = 64 * 1024;

pub fn restore_from_sql(
    conn: &Connection,
    file_path: &str,
    continue_on_error: bool,
    cancelled: &AtomicBool,
    on_progress: &dyn Fn(RestoreProgress),
) -> Result<RestoreProgress, SakiError> {
    info!(file_path, continue_on_error, "starting SQL restore");

    let metadata = std::fs::metadata(file_path)
        .map_err(|e| SakiError::QueryFailed(format!("Cannot read file: {e}")))?;
    let total_bytes = metadata.len();

    let file = std::fs::File::open(file_path)
        .map_err(|e| SakiError::QueryFailed(format!("Cannot open file: {e}")))?;
    let mut reader = BufReader::with_capacity(READ_BUF_SIZE, file);

    let start = Instant::now();
    let mut progress = RestoreProgress {
        bytes_read: 0,
        total_bytes,
        statements_executed: 0,
        errors_skipped: 0,
        phase: "Executing".to_string(),
        elapsed_ms: 0,
        error: None,
        error_messages: Vec::new(),
    };

    let mut splitter = StreamingSqlSplitter::new(SqlSplitOptions::default());
    let mut batch: Vec<String> = Vec::with_capacity(BATCH_SIZE);
    let mut last_progress = Instant::now();

    // Stream file line-by-line, feeding each line to the incremental parser.
    loop {
        if cancelled.load(Ordering::Relaxed) {
            progress.phase = "Cancelled".to_string();
            progress.elapsed_ms = start.elapsed().as_millis() as u64;
            on_progress(progress.clone());
            return Err(SakiError::Cancelled);
        }

        let mut line = String::new();
        let bytes_read = reader
            .read_line(&mut line)
            .map_err(|e| SakiError::QueryFailed(format!("Read error: {e}")))?;

        if bytes_read == 0 {
            // EOF — flush the splitter's remaining buffer
            if let Some(stmt) = splitter.finish() {
                batch.push(stmt);
            }
            break;
        }

        progress.bytes_read += bytes_read as u64;

        // Feed the line to the streaming parser; collect any complete statements.
        let stmts = splitter.feed(&line);
        for stmt in stmts {
            batch.push(stmt);

            if batch.len() >= BATCH_SIZE {
                flush_batch(conn, &mut batch, continue_on_error, &mut progress)?;
            }
        }

        if last_progress.elapsed().as_millis() > 100 {
            progress.elapsed_ms = start.elapsed().as_millis() as u64;
            on_progress(progress.clone());
            last_progress = Instant::now();
        }
    }

    // Flush remaining
    if !batch.is_empty() {
        flush_batch(conn, &mut batch, continue_on_error, &mut progress)?;
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

fn flush_batch(
    conn: &Connection,
    batch: &mut Vec<String>,
    continue_on_error: bool,
    progress: &mut RestoreProgress,
) -> Result<(), SakiError> {
    if batch.is_empty() {
        return Ok(());
    }

    let sql = batch.join(";\n");
    match conn.execute_batch(&sql) {
        Ok(()) => {
            progress.statements_executed += batch.len() as u64;
        }
        Err(e) => {
            if !continue_on_error {
                batch.clear();
                return Err(SakiError::QueryFailed(e.to_string()));
            }
            warn!(batch_size = batch.len(), "batch failed, retrying one-by-one");
            for stmt in batch.iter() {
                match conn.execute_batch(stmt) {
                    Ok(()) => progress.statements_executed += 1,
                    Err(e) => {
                        progress.errors_skipped += 1;
                        if progress.error_messages.len() < MAX_ERROR_MESSAGES {
                            let label: String = stmt.chars().take(80).collect();
                            progress
                                .error_messages
                                .push(format!("{label}... → {e}"));
                        }
                    }
                }
            }
        }
    }

    batch.clear();
    Ok(())
}
