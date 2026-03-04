use std::fmt::Write as FmtWrite;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tracing::{error, info, warn};

use sakidb_core::types::*;
use sakidb_core::{DatabaseDriver, SakiError};
use sakidb_postgres::executor;

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
    let pool = state.driver.get_pool(conn_id).await.ok()?;
    let client = pool.get().await.ok()?;
    let count_sql = format!("SELECT COUNT(*) FROM ({sql}) AS _cnt");
    let row = client.query_opt(&count_sql, &[]).await.ok()??;
    row.get(0)
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
    let mut writer = std::io::BufWriter::new(file);
    let mut line_buf = String::with_capacity(4096);
    let mut header_written = false;

    let pool = state
        .driver
        .get_pool(&conn_id)
        .await
        .map_err(|e| e.to_string())?;

    let result = executor::execute_export_cursor(
        &pool,
        &base_sql,
        1_000,
        &mut |columns, cells, total_rows| {
            // Write header on first batch
            if !header_written && include_header && !columns.is_empty() {
                line_buf.clear();
                for (i, col) in columns.iter().enumerate() {
                    if i > 0 {
                        line_buf.push(',');
                    }
                    write_csv_cell(&mut line_buf, &CellValue::Text(Box::from(col.name.as_str())));
                }
                line_buf.push('\n');
                writer
                    .write_all(line_buf.as_bytes())
                    .map_err(|e| SakiError::QueryFailed(format!("Write error: {e}")))?;
                header_written = true;
            }

            let num_cols = columns.len();
            let row_count = if num_cols > 0 { cells.len() / num_cols } else { 0 };

            for row_idx in 0..row_count {
                line_buf.clear();
                for col_idx in 0..num_cols {
                    if col_idx > 0 {
                        line_buf.push(',');
                    }
                    write_csv_cell(&mut line_buf, &cells[row_idx * num_cols + col_idx]);
                }
                line_buf.push('\n');
                writer
                    .write_all(line_buf.as_bytes())
                    .map_err(|e| SakiError::QueryFailed(format!("Write error: {e}")))?;
            }

            // Emit progress
            let _ = app_handle.emit(
                "export-progress",
                ExportProgress {
                    rows_exported: total_rows,
                    total_rows_estimate: total_estimate,
                    phase: "exporting".to_string(),
                },
            );

            Ok(())
        },
        &cancel_flag,
    )
    .await;

    // Cleanup cancel flag
    state.export_cancel_flags.remove(&conn_id);

    match result {
        Ok(total_rows) => {
            writer.flush().map_err(|e| format!("Flush error: {e}"))?;
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
            let _ = writer.flush();
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

    let mut file = std::fs::File::create(&file_path)
        .map_err(|e| format!("Failed to create file: {e}"))?;
    let mut writer = std::io::BufWriter::new(&mut file);

    let qualified = format!("{}.{}", quote_ident(&schema), quote_ident(&table));

    // Phase 1: DDL
    if include_ddl {
        let columns = state
            .driver
            .list_columns(&conn_id, &schema, &table)
            .await
            .map_err(|e| e.to_string())?;

        let unique_constraints = state
            .driver
            .list_unique_constraints(&conn_id, &schema, &table)
            .await
            .map_err(|e| e.to_string())?;

        let foreign_keys = state
            .driver
            .list_foreign_keys(&conn_id, &schema, &table)
            .await
            .map_err(|e| e.to_string())?;

        let check_constraints = state
            .driver
            .list_check_constraints(&conn_id, &schema, &table)
            .await
            .map_err(|e| e.to_string())?;

        writeln!(writer, "CREATE TABLE {qualified} (")
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

        writeln!(writer, "{}\n);\n", col_defs.join(",\n"))
            .map_err(|e| format!("Write error: {e}"))?;

        let indexes = state
            .driver
            .list_indexes(&conn_id, &schema)
            .await
            .map_err(|e| e.to_string())?;

        for idx in indexes
            .iter()
            .filter(|i| i.table_name == table && !i.is_primary)
        {
            let unique = if idx.is_unique { "UNIQUE " } else { "" };
            writeln!(
                writer,
                "CREATE {unique}INDEX {} ON {qualified} USING {} ({});\n",
                quote_ident(&idx.name),
                idx.index_type,
                idx.columns
            )
            .map_err(|e| format!("Write error: {e}"))?;
        }

        let triggers = state
            .driver
            .list_triggers(&conn_id, &schema, &table)
            .await
            .map_err(|e| e.to_string())?;

        for trig in &triggers {
            let condition = trig
                .condition
                .as_ref()
                .map(|c| format!("\n    WHEN ({c})"))
                .unwrap_or_default();
            writeln!(
                writer,
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

        let mut line_buf = String::with_capacity(4096);
        let mut copy_header_written = false;

        let pool = state
            .driver
            .get_pool(&conn_id)
            .await
            .map_err(|e| e.to_string())?;

        let result = executor::execute_export_cursor(
            &pool,
            &sql,
            1_000,
            &mut |columns, cells, rows_so_far| {
                // Write COPY header on first batch
                if !copy_header_written && !columns.is_empty() {
                    let col_names: Vec<String> =
                        columns.iter().map(|c| quote_ident(&c.name)).collect();
                    writeln!(writer, "COPY {qualified} ({}) FROM stdin;", col_names.join(", "))
                        .map_err(|e| SakiError::QueryFailed(format!("Write error: {e}")))?;
                    copy_header_written = true;
                }

                let num_cols = columns.len();
                let row_count = if num_cols > 0 { cells.len() / num_cols } else { 0 };

                for row_idx in 0..row_count {
                    line_buf.clear();
                    for col_idx in 0..num_cols {
                        if col_idx > 0 {
                            line_buf.push('\t');
                        }
                        write_copy_cell(&mut line_buf, &cells[row_idx * num_cols + col_idx]);
                    }
                    line_buf.push('\n');
                    writer
                        .write_all(line_buf.as_bytes())
                        .map_err(|e| SakiError::QueryFailed(format!("Write error: {e}")))?;
                }

                let _ = app_handle.emit(
                    "export-progress",
                    ExportProgress {
                        rows_exported: rows_so_far,
                        total_rows_estimate: total_estimate,
                        phase: "exporting".to_string(),
                    },
                );

                Ok(())
            },
            &cancel_flag,
        )
        .await;

        state.export_cancel_flags.remove(&conn_id);

        match result {
            Ok(rows) => {
                // Write COPY terminator
                if copy_header_written {
                    writeln!(writer, "\\.").map_err(|e| format!("Write error: {e}"))?;
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
                let _ = writer.flush();
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

    writer.flush().map_err(|e| format!("Flush error: {e}"))?;
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
