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

fn qualified_table(schema: &str, table: &str) -> String {
    if schema.is_empty() {
        quote_ident(table)
    } else {
        format!("{}.{}", quote_ident(schema), quote_ident(table))
    }
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

    let qt = qualified_table(&schema, &table);
    let base_sql = match &where_clause {
        Some(wc) if !wc.is_empty() => format!("SELECT * FROM {qt} WHERE {wc}"),
        _ => format!("SELECT * FROM {qt}"),
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

    let qualified = qualified_table(&schema, &table);

    // Phase 1: DDL — use SqlFormatter if available, else fall back to get_create_table_sql
    if include_ddl {
        let introspector = state.registry.introspector_for(&conn_id).map_err(|e| e.to_string())?;
        let formatter = state.registry.formatter_for(&conn_id).ok();

        // Try formatter-generated DDL first (uses introspected metadata)
        let ddl_text = if let Some(fmt) = formatter {
            let columns = introspector.list_columns(&conn_id, &schema, &table).await.map_err(|e| e.to_string())?;
            let indexes = introspector.list_indexes(&conn_id, &schema).await.map_err(|e| e.to_string())?;
            let constraints = introspector.list_unique_constraints(&conn_id, &schema, &table).await.map_err(|e| e.to_string())?;
            let foreign_keys = introspector.list_foreign_keys(&conn_id, &schema, &table).await.map_err(|e| e.to_string())?;
            let check_constraints = introspector.list_check_constraints(&conn_id, &schema, &table).await.map_err(|e| e.to_string())?;
            let triggers = introspector.list_triggers(&conn_id, &schema, &table).await.map_err(|e| e.to_string())?;

            fmt.format_ddl(&DdlContext {
                columns: &columns,
                indexes: &indexes,
                constraints: &constraints,
                foreign_keys: &foreign_keys,
                check_constraints: &check_constraints,
                triggers: &triggers,
                qualified_table: &qualified,
                table_name: &table,
            })
        } else {
            None
        };

        // Fall back to engine's native CREATE TABLE SQL (e.g. SQLite's sqlite_master)
        let ddl_text = match ddl_text {
            Some(ddl) => ddl,
            None => introspector.get_create_table_sql(&conn_id, &schema, &table).await.map_err(|e| e.to_string())?,
        };

        {
            let mut w = writer.lock().unwrap();
            writeln!(w, "{ddl_text}").map_err(|e| format!("Write error: {e}"))?;
        }
    }

    // Phase 2: Data using engine-specific format via SqlFormatter
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

        let formatter = state.registry.formatter_arc_for(&conn_id).map_err(|e| e.to_string())?;

        let data_state = Arc::new(Mutex::new((String::with_capacity(4096), false))); // (line_buf, header_written)
        let data_state_cb = data_state.clone();
        let writer_cb = writer.clone();
        let app_clone = app_handle.clone();
        let qualified_clone = qualified.clone();
        let formatter_cb = formatter.clone();

        let on_batch = move |columns: &[ColumnDef], cells: &[CellValue], rows_so_far: u64| -> sakidb_core::error::Result<()> {
            let mut ds = data_state_cb.lock().unwrap();
            let (ref mut line_buf, ref mut header_written) = *ds;
            let mut w = writer_cb.lock().unwrap();

            // Write data header on first batch (e.g. COPY ... FROM stdin; for Postgres)
            if !*header_written && !columns.is_empty() {
                if let Some(header) = formatter_cb.format_data_header(columns, &qualified_clone) {
                    w.write_all(header.as_bytes())
                        .map_err(|e| SakiError::QueryFailed(format!("Write error: {e}")))?;
                }
                *header_written = true;
            }

            let num_cols = columns.len();
            let row_count = if num_cols > 0 { cells.len() / num_cols } else { 0 };

            for row_idx in 0..row_count {
                line_buf.clear();
                let row_cells = &cells[row_idx * num_cols..(row_idx + 1) * num_cols];
                formatter_cb.format_data_row(columns, row_cells, &qualified_clone, line_buf);
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
                // Write data footer (e.g. \. for Postgres COPY)
                let ds = data_state.lock().unwrap();
                if ds.1 {
                    if let Some(footer) = formatter.format_data_footer() {
                        let mut w = writer.lock().unwrap();
                        w.write_all(footer.as_bytes())
                            .map_err(|e| format!("Write error: {e}"))?;
                    }
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
