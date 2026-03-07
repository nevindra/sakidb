use std::fmt::Write as FmtWrite;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tracing::{error, info, warn};

use sakidb_core::types::*;
use sakidb_core::SakiError;

use crate::state::AppState;

fn parse_conn_id(id: &str) -> Result<ConnectionId, String> {
    Ok(ConnectionId(
        uuid::Uuid::parse_str(id).map_err(|e| e.to_string())?,
    ))
}

#[derive(Clone, Serialize)]
struct ExportProgress {
    rows_exported: u64,
    total_rows_estimate: Option<i64>,
    phase: String,
}

fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// Write a CellValue into a CSV field directly into the buffer. No intermediate allocations.
fn write_csv_cell(buf: &mut String, cell: &CellValue) {
    match cell {
        CellValue::Null => {}
        CellValue::Bool(b) => {
            let _ = write!(buf, "{b}");
        }
        CellValue::Int(i) => {
            let _ = write!(buf, "{i}");
        }
        CellValue::Float(f) => {
            let _ = write!(buf, "{f}");
        }
        CellValue::Text(s) | CellValue::Json(s) | CellValue::Timestamp(s) => {
            if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
                buf.push('"');
                for ch in s.chars() {
                    if ch == '"' {
                        buf.push('"');
                    }
                    buf.push(ch);
                }
                buf.push('"');
            } else {
                buf.push_str(s);
            }
        }
        CellValue::Bytes(b) => {
            buf.push_str("\\x");
            for byte in b {
                let _ = write!(buf, "{byte:02x}");
            }
        }
    }
}

/// Write a CellValue in PostgreSQL COPY text format into the buffer.
fn write_copy_cell(buf: &mut String, cell: &CellValue) {
    match cell {
        CellValue::Null => buf.push_str("\\N"),
        CellValue::Bool(b) => {
            let _ = write!(buf, "{b}");
        }
        CellValue::Int(i) => {
            let _ = write!(buf, "{i}");
        }
        CellValue::Float(f) => {
            let _ = write!(buf, "{f}");
        }
        CellValue::Text(s) | CellValue::Json(s) | CellValue::Timestamp(s) => {
            for ch in s.chars() {
                match ch {
                    '\\' => buf.push_str("\\\\"),
                    '\t' => buf.push_str("\\t"),
                    '\n' => buf.push_str("\\n"),
                    '\r' => buf.push_str("\\r"),
                    _ => buf.push(ch),
                }
            }
        }
        CellValue::Bytes(b) => {
            buf.push_str("\\\\x");
            for byte in b {
                let _ = write!(buf, "{byte:02x}");
            }
        }
    }
}

/// Run a single COUNT(*) query. Returns None on failure (non-fatal).
async fn count_estimate(state: &AppState, conn_id: &ConnectionId, sql: &str) -> Option<i64> {
    let sql_driver = state.registry.sql_for(conn_id).ok()?;
    let count_sql = format!("SELECT COUNT(*) FROM ({sql}) AS _cnt");
    let result = sql_driver.execute(conn_id, &count_sql).await.ok()?;
    if !result.cells.is_empty() {
        match &result.cells[0] {
            CellValue::Int(i) => Some(*i),
            _ => None,
        }
    } else {
        None
    }
}

#[tauri::command]
pub async fn export_table_csv(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
    table: String,
    file_path: String,
    where_clause: Option<String>,
    include_header: bool,
) -> Result<u64, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    info!(schema = %schema, table = %table, file_path = %file_path, "starting CSV export");

    let base_sql = match &where_clause {
        Some(wc) if !wc.is_empty() => {
            format!(
                "SELECT * FROM {}.{} WHERE {wc}",
                quote_ident(&schema),
                quote_ident(&table)
            )
        }
        _ => format!("SELECT * FROM {}.{}", quote_ident(&schema), quote_ident(&table)),
    };

    // Optional count for progress
    let total_estimate = count_estimate(&state, &conn_id, &base_sql).await;

    // Set up cancel flag
    let cancel_flag = Arc::new(AtomicBool::new(false));
    state
        .export_cancel_flags
        .insert(conn_id, cancel_flag.clone());

    let file = std::fs::File::create(&file_path)
        .map_err(|e| format!("Failed to create file: {e}"))?;
    let writer = std::io::BufWriter::new(file);

    // Wrap mutable state in Arc<Mutex> so callback can move a clone
    let export_state = Arc::new(Mutex::new((writer, String::with_capacity(4096), false))); // (writer, line_buf, header_written)
    let export_state_cb = export_state.clone();
    let app_clone = app_handle.clone();

    let on_batch = move |columns: &[ColumnDef], cells: &[CellValue], total_rows: u64| -> sakidb_core::error::Result<()> {
        let mut guard = export_state_cb.lock().unwrap();
        let (ref mut writer, ref mut line_buf, ref mut header_written) = *guard;

        // Write header on first batch
        if !*header_written && include_header && !columns.is_empty() {
            line_buf.clear();
            for (i, col) in columns.iter().enumerate() {
                if i > 0 {
                    line_buf.push(',');
                }
                write_csv_cell(line_buf, &CellValue::Text(Box::from(col.name.as_str())));
            }
            line_buf.push('\n');
            writer
                .write_all(line_buf.as_bytes())
                .map_err(|e| SakiError::QueryFailed(format!("Write error: {e}")))?;
            *header_written = true;
        }

        let num_cols = columns.len();
        let row_count = if num_cols > 0 { cells.len() / num_cols } else { 0 };

        for row_idx in 0..row_count {
            line_buf.clear();
            for col_idx in 0..num_cols {
                if col_idx > 0 {
                    line_buf.push(',');
                }
                write_csv_cell(line_buf, &cells[row_idx * num_cols + col_idx]);
            }
            line_buf.push('\n');
            writer
                .write_all(line_buf.as_bytes())
                .map_err(|e| SakiError::QueryFailed(format!("Write error: {e}")))?;
        }

        // Emit progress
        let _ = app_clone.emit(
            "export-progress",
            ExportProgress {
                rows_exported: total_rows,
                total_rows_estimate: total_estimate,
                phase: "exporting".to_string(),
            },
        );

        Ok(())
    };

    let exporter = state.registry.exporter_for(&conn_id).map_err(|e| e.to_string())?;
    let result = exporter
        .export_stream(&conn_id, &base_sql, 1_000, &cancel_flag, &on_batch)
        .await;

    // Cleanup cancel flag
    state.export_cancel_flags.remove(&conn_id);

    match result {
        Ok(total_rows) => {
            let mut guard = export_state.lock().unwrap();
            guard.0.flush().map_err(|e| format!("Flush error: {e}"))?;
            info!(rows = total_rows, file_path = %file_path, "CSV export complete");
            let _ = app_handle.emit(
                "export-progress",
                ExportProgress {
                    rows_exported: total_rows,
                    total_rows_estimate: total_estimate,
                    phase: "complete".to_string(),
                },
            );
            Ok(total_rows)
        }
        Err(SakiError::Cancelled) => {
            let _ = export_state.lock().unwrap().0.flush();
            warn!(file_path = %file_path, "CSV export cancelled");
            let _ = app_handle.emit(
                "export-progress",
                ExportProgress {
                    rows_exported: 0,
                    total_rows_estimate: total_estimate,
                    phase: "cancelled".to_string(),
                },
            );
            Err("Export cancelled".to_string())
        }
        Err(e) => {
            error!(file_path = %file_path, error = %e, "CSV export failed");
            let _ = app_handle.emit(
                "export-progress",
                ExportProgress {
                    rows_exported: 0,
                    total_rows_estimate: total_estimate,
                    phase: "error".to_string(),
                },
            );
            Err(e.to_string())
        }
    }
}

#[tauri::command]
pub async fn export_table_sql(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
    table: String,
    file_path: String,
    include_ddl: bool,
    include_data: bool,
) -> Result<u64, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    info!(schema = %schema, table = %table, file_path = %file_path, include_ddl, include_data, "starting SQL export");

    let file = std::fs::File::create(&file_path)
        .map_err(|e| format!("Failed to create file: {e}"))?;
    let writer = Arc::new(Mutex::new(std::io::BufWriter::new(file)));

    let qualified = format!("{}.{}", quote_ident(&schema), quote_ident(&table));

    // Phase 1: DDL — fetch all metadata first (async), then write synchronously
    if include_ddl {
        let introspector = state.registry.introspector_for(&conn_id).map_err(|e| e.to_string())?;

        let columns = introspector
            .list_columns(&conn_id, &schema, &table)
            .await
            .map_err(|e| e.to_string())?;

        let unique_constraints = introspector
            .list_unique_constraints(&conn_id, &schema, &table)
            .await
            .map_err(|e| e.to_string())?;

        let foreign_keys = introspector
            .list_foreign_keys(&conn_id, &schema, &table)
            .await
            .map_err(|e| e.to_string())?;

        let check_constraints = introspector
            .list_check_constraints(&conn_id, &schema, &table)
            .await
            .map_err(|e| e.to_string())?;

        let indexes = introspector
            .list_indexes(&conn_id, &schema)
            .await
            .map_err(|e| e.to_string())?;

        let triggers = introspector
            .list_triggers(&conn_id, &schema, &table)
            .await
            .map_err(|e| e.to_string())?;

        // All async fetches done — now write DDL synchronously (no await while holding lock)
        {
            let mut w = writer.lock().unwrap();

            writeln!(w, "CREATE TABLE {qualified} (")
                .map_err(|e| format!("Write error: {e}"))?;

            let mut col_defs: Vec<String> = Vec::new();
            for col in &columns {
                let mut def = format!("    {} {}", quote_ident(&col.name), col.data_type);
                if !col.is_nullable {
                    def.push_str(" NOT NULL");
                }
                if let Some(ref default) = col.default_value {
                    def.push_str(&format!(" DEFAULT {default}"));
                }
                col_defs.push(def);
            }

            for uc in &unique_constraints {
                let cols: Vec<String> = uc.columns.iter().map(|c| quote_ident(c)).collect();
                if uc.is_primary {
                    col_defs.push(format!(
                        "    CONSTRAINT {} PRIMARY KEY ({})",
                        quote_ident(&uc.constraint_name),
                        cols.join(", ")
                    ));
                } else {
                    col_defs.push(format!(
                        "    CONSTRAINT {} UNIQUE ({})",
                        quote_ident(&uc.constraint_name),
                        cols.join(", ")
                    ));
                }
            }

            for fk in &foreign_keys {
                let local_cols: Vec<String> = fk.columns.iter().map(|c| quote_ident(c)).collect();
                let foreign_cols: Vec<String> =
                    fk.foreign_columns.iter().map(|c| quote_ident(c)).collect();
                col_defs.push(format!(
                    "    CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {}.{} ({}) ON UPDATE {} ON DELETE {}",
                    quote_ident(&fk.constraint_name),
                    local_cols.join(", "),
                    quote_ident(&fk.foreign_table_schema),
                    quote_ident(&fk.foreign_table_name),
                    foreign_cols.join(", "),
                    fk.on_update,
                    fk.on_delete
                ));
            }

            for cc in &check_constraints {
                col_defs.push(format!(
                    "    CONSTRAINT {} {}",
                    quote_ident(&cc.constraint_name),
                    cc.check_clause
                ));
            }

            writeln!(w, "{}\n);\n", col_defs.join(",\n"))
                .map_err(|e| format!("Write error: {e}"))?;

            for idx in indexes
                .iter()
                .filter(|i| i.table_name == table && !i.is_primary)
            {
                let unique = if idx.is_unique { "UNIQUE " } else { "" };
                writeln!(
                    w,
                    "CREATE {unique}INDEX {} ON {qualified} USING {} ({});\n",
                    quote_ident(&idx.name),
                    idx.index_type,
                    idx.columns
                )
                .map_err(|e| format!("Write error: {e}"))?;
            }

            for trig in &triggers {
                let condition = trig
                    .condition
                    .as_ref()
                    .map(|c| format!("\n    WHEN ({c})"))
                    .unwrap_or_default();
                writeln!(
                    w,
                    "CREATE TRIGGER {} {} {} ON {qualified}\n    FOR EACH {}{}\n    EXECUTE FUNCTION {}.{}();\n",
                    quote_ident(&trig.name),
                    trig.timing,
                    trig.event,
                    trig.for_each,
                    condition,
                    quote_ident(&trig.function_schema),
                    quote_ident(&trig.function_name)
                )
                .map_err(|e| format!("Write error: {e}"))?;
            }
        } // MutexGuard dropped here
    }

    // Phase 2: Data using COPY format
    let mut total_rows: u64 = 0;

    if include_data {
        let sql = format!("SELECT * FROM {qualified}");

        // Optional count for progress
        let total_estimate = count_estimate(&state, &conn_id, &sql).await;

        // Set up cancel flag
        let cancel_flag = Arc::new(AtomicBool::new(false));
        state
            .export_cancel_flags
            .insert(conn_id, cancel_flag.clone());

        let copy_state = Arc::new(Mutex::new((String::with_capacity(4096), false))); // (line_buf, copy_header_written)
        let copy_state_cb = copy_state.clone();
        let writer_cb = writer.clone();
        let app_clone = app_handle.clone();
        let qualified_clone = qualified.clone();

        let on_batch = move |columns: &[ColumnDef], cells: &[CellValue], rows_so_far: u64| -> sakidb_core::error::Result<()> {
            let mut cs = copy_state_cb.lock().unwrap();
            let (ref mut line_buf, ref mut copy_header_written) = *cs;
            let mut w = writer_cb.lock().unwrap();

            // Write COPY header on first batch
            if !*copy_header_written && !columns.is_empty() {
                let col_names: Vec<String> =
                    columns.iter().map(|c| quote_ident(&c.name)).collect();
                writeln!(w, "COPY {qualified_clone} ({}) FROM stdin;", col_names.join(", "))
                    .map_err(|e| SakiError::QueryFailed(format!("Write error: {e}")))?;
                *copy_header_written = true;
            }

            let num_cols = columns.len();
            let row_count = if num_cols > 0 { cells.len() / num_cols } else { 0 };

            for row_idx in 0..row_count {
                line_buf.clear();
                for col_idx in 0..num_cols {
                    if col_idx > 0 {
                        line_buf.push('\t');
                    }
                    write_copy_cell(line_buf, &cells[row_idx * num_cols + col_idx]);
                }
                line_buf.push('\n');
                w.write_all(line_buf.as_bytes())
                    .map_err(|e| SakiError::QueryFailed(format!("Write error: {e}")))?;
            }

            let _ = app_clone.emit(
                "export-progress",
                ExportProgress {
                    rows_exported: rows_so_far,
                    total_rows_estimate: total_estimate,
                    phase: "exporting".to_string(),
                },
            );

            Ok(())
        };

        let exporter = state.registry.exporter_for(&conn_id).map_err(|e| e.to_string())?;
        let result = exporter
            .export_stream(&conn_id, &sql, 1_000, &cancel_flag, &on_batch)
            .await;

        state.export_cancel_flags.remove(&conn_id);

        match result {
            Ok(rows) => {
                // Write COPY terminator
                let cs = copy_state.lock().unwrap();
                if cs.1 {
                    let mut w = writer.lock().unwrap();
                    writeln!(w, "\\.").map_err(|e| format!("Write error: {e}"))?;
                }
                total_rows = rows;
                let _ = app_handle.emit(
                    "export-progress",
                    ExportProgress {
                        rows_exported: total_rows,
                        total_rows_estimate: total_estimate,
                        phase: "complete".to_string(),
                    },
                );
            }
            Err(SakiError::Cancelled) => {
                let _ = writer.lock().unwrap().flush();
                let _ = app_handle.emit(
                    "export-progress",
                    ExportProgress {
                        rows_exported: 0,
                        total_rows_estimate: total_estimate,
                        phase: "cancelled".to_string(),
                    },
                );
                return Err("Export cancelled".to_string());
            }
            Err(e) => {
                let _ = app_handle.emit(
                    "export-progress",
                    ExportProgress {
                        rows_exported: 0,
                        total_rows_estimate: total_estimate,
                        phase: "error".to_string(),
                    },
                );
                return Err(e.to_string());
            }
        }
    }

    writer.lock().unwrap().flush().map_err(|e| format!("Flush error: {e}"))?;
    info!(rows = total_rows, file_path = %file_path, "SQL export complete");
    Ok(total_rows)
}

#[tauri::command]
pub async fn cancel_export(
    state: State<'_, AppState>,
    active_connection_id: String,
) -> Result<(), String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    if let Some(flag) = state.export_cancel_flags.get(&conn_id) {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}
