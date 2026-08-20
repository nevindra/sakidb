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

type SqlWriter = Arc<Mutex<std::io::BufWriter<std::fs::File>>>;

/// Generate CREATE TABLE (+ indexes/constraints/triggers) DDL for one table.
/// Prefers the engine's SqlFormatter over introspected metadata, falling back
/// to the engine's native CREATE TABLE SQL (e.g. SQLite's sqlite_master).
async fn build_table_ddl(
    state: &AppState,
    conn_id: &ConnectionId,
    schema: &str,
    table: &str,
    qualified: &str,
) -> Result<String, SakiError> {
    let introspector = state.registry.introspector_for(conn_id)?;
    let formatter = state.registry.formatter_for(conn_id).ok();

    let ddl_text = if let Some(fmt) = formatter {
        let columns = introspector.list_columns(conn_id, schema, table).await?;
        let indexes = introspector.list_indexes(conn_id, schema).await?;
        let constraints = introspector
            .list_unique_constraints(conn_id, schema, table)
            .await?;
        let foreign_keys = introspector
            .list_foreign_keys(conn_id, schema, table)
            .await?;
        let check_constraints = introspector
            .list_check_constraints(conn_id, schema, table)
            .await?;
        let triggers = introspector.list_triggers(conn_id, schema, table).await?;

        fmt.format_ddl(&DdlContext {
            columns: &columns,
            indexes: &indexes,
            constraints: &constraints,
            foreign_keys: &foreign_keys,
            check_constraints: &check_constraints,
            triggers: &triggers,
            qualified_table: qualified,
            table_name: table,
        })
    } else {
        None
    };

    match ddl_text {
        Some(ddl) => Ok(ddl),
        None => {
            introspector
                .get_create_table_sql(conn_id, schema, table)
                .await
        }
    }
}

/// Stream one table's rows into `writer` in the engine's SQL data format
/// (header + rows + footer). Emits `export-progress` events with row counts
/// offset by `rows_base` so multi-table exports report a cumulative total.
#[allow(clippy::too_many_arguments)]
async fn stream_table_data(
    app_handle: &AppHandle,
    state: &AppState,
    conn_id: &ConnectionId,
    qualified: &str,
    writer: &SqlWriter,
    cancel_flag: &Arc<AtomicBool>,
    rows_base: u64,
    total_estimate: Option<i64>,
) -> Result<u64, SakiError> {
    let sql = format!("SELECT * FROM {qualified}");
    let formatter = state.registry.formatter_arc_for(conn_id)?;

    let data_state = Arc::new(Mutex::new((String::with_capacity(4096), false))); // (line_buf, header_written)
    let data_state_cb = data_state.clone();
    let writer_cb = writer.clone();
    let app_clone = app_handle.clone();
    let qualified_clone = qualified.to_string();
    let formatter_cb = formatter.clone();

    let on_batch = move |columns: &[ColumnDef],
                         cells: &[CellValue],
                         rows_so_far: u64|
          -> sakidb_core::error::Result<()> {
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
        let row_count = cells.len().checked_div(num_cols).unwrap_or(0);

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
                rows_exported: rows_base + rows_so_far,
                total_rows_estimate: total_estimate,
                phase: "exporting".to_string(),
            },
        );

        Ok(())
    };

    let exporter = state.registry.exporter_for(conn_id)?;
    let rows = exporter
        .export_stream(conn_id, &sql, 1_000, cancel_flag, &on_batch)
        .await?;

    // Write data footer (e.g. \. for Postgres COPY)
    let ds = data_state.lock().unwrap();
    if ds.1 {
        if let Some(footer) = formatter.format_data_footer() {
            let mut w = writer.lock().unwrap();
            w.write_all(footer.as_bytes())
                .map_err(|e| SakiError::QueryFailed(format!("Write error: {e}")))?;
        }
    }

    Ok(rows)
}

fn emit_progress(app_handle: &AppHandle, rows: u64, estimate: Option<i64>, phase: &str) {
    let _ = app_handle.emit(
        "export-progress",
        ExportProgress {
            rows_exported: rows,
            total_rows_estimate: estimate,
            phase: phase.to_string(),
        },
    );
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
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

    let file =
        std::fs::File::create(&file_path).map_err(|e| format!("Failed to create file: {e}"))?;
    let writer = std::io::BufWriter::new(file);

    // Wrap mutable state in Arc<Mutex> so callback can move a clone
    let export_state = Arc::new(Mutex::new((writer, String::with_capacity(4096), false))); // (writer, line_buf, header_written)
    let export_state_cb = export_state.clone();
    let app_clone = app_handle.clone();

    let on_batch = move |columns: &[ColumnDef],
                         cells: &[CellValue],
                         total_rows: u64|
          -> sakidb_core::error::Result<()> {
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
        let row_count = cells.len().checked_div(num_cols).unwrap_or(0);

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

    let exporter = state
        .registry
        .exporter_for(&conn_id)
        .map_err(|e| e.to_string())?;
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
#[allow(clippy::too_many_arguments)]
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

    let file =
        std::fs::File::create(&file_path).map_err(|e| format!("Failed to create file: {e}"))?;
    let writer: SqlWriter = Arc::new(Mutex::new(std::io::BufWriter::new(file)));

    let qualified = qualified_table(&schema, &table);

    if include_ddl {
        let ddl_text = build_table_ddl(&state, &conn_id, &schema, &table, &qualified)
            .await
            .map_err(|e| e.to_string())?;
        let mut w = writer.lock().unwrap();
        writeln!(w, "{ddl_text}").map_err(|e| format!("Write error: {e}"))?;
    }

    let mut total_rows: u64 = 0;

    if include_data {
        let count_sql = format!("SELECT * FROM {qualified}");
        let total_estimate = count_estimate(&state, &conn_id, &count_sql).await;

        let cancel_flag = Arc::new(AtomicBool::new(false));
        state
            .export_cancel_flags
            .insert(conn_id, cancel_flag.clone());

        let result = stream_table_data(
            &app_handle,
            &state,
            &conn_id,
            &qualified,
            &writer,
            &cancel_flag,
            0,
            total_estimate,
        )
        .await;

        state.export_cancel_flags.remove(&conn_id);

        match result {
            Ok(rows) => {
                total_rows = rows;
                emit_progress(&app_handle, total_rows, total_estimate, "complete");
            }
            Err(SakiError::Cancelled) => {
                let _ = writer.lock().unwrap().flush();
                emit_progress(&app_handle, 0, total_estimate, "cancelled");
                return Err("Export cancelled".to_string());
            }
            Err(e) => {
                emit_progress(&app_handle, 0, total_estimate, "error");
                return Err(e.to_string());
            }
        }
    }

    writer
        .lock()
        .unwrap()
        .flush()
        .map_err(|e| format!("Flush error: {e}"))?;
    info!(rows = total_rows, file_path = %file_path, "SQL export complete");
    Ok(total_rows)
}

#[tauri::command]
pub async fn export_database_sql(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    active_connection_id: String,
    file_path: String,
    include_ddl: bool,
    include_data: bool,
    schema: Option<String>,
) -> Result<u64, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    info!(file_path = %file_path, schema = ?schema, include_ddl, include_data, "starting database SQL export");

    const SYSTEM_SCHEMAS: &[&str] = &["pg_catalog", "information_schema", "pg_toast"];

    let introspector = state
        .registry
        .introspector_for(&conn_id)
        .map_err(|e| e.to_string())?;

    let schemas: Vec<String> = match &schema {
        Some(s) => vec![s.clone()],
        None => {
            let all = introspector
                .list_schemas(&conn_id)
                .await
                .map_err(|e| e.to_string())?;
            let filtered: Vec<String> = all
                .into_iter()
                .map(|s| s.name)
                .filter(|n| !SYSTEM_SCHEMAS.contains(&n.as_str()))
                .collect();
            if filtered.is_empty() {
                // Engines without schema support export from the unqualified namespace
                vec![String::new()]
            } else {
                filtered
            }
        }
    };

    // Partition children are skipped: the parent's data stream already contains
    // their rows, and re-inserting them through the parent would duplicate data.
    let mut targets: Vec<(String, String)> = Vec::new();
    for s in &schemas {
        let tables = introspector
            .list_tables(&conn_id, s)
            .await
            .map_err(|e| e.to_string())?;
        for t in tables.into_iter().filter(|t| !t.is_partition) {
            targets.push((s.clone(), t.name));
        }
    }

    let file =
        std::fs::File::create(&file_path).map_err(|e| format!("Failed to create file: {e}"))?;
    let writer: SqlWriter = Arc::new(Mutex::new(std::io::BufWriter::new(file)));

    let cancel_flag = Arc::new(AtomicBool::new(false));
    state
        .export_cancel_flags
        .insert(conn_id, cancel_flag.clone());

    let mut total_rows: u64 = 0;
    let result: Result<(), SakiError> = async {
        for (s, t) in &targets {
            if cancel_flag.load(Ordering::Relaxed) {
                return Err(SakiError::Cancelled);
            }
            let qualified = qualified_table(s, t);
            if include_ddl {
                let ddl_text = build_table_ddl(&state, &conn_id, s, t, &qualified).await?;
                let mut w = writer.lock().unwrap();
                writeln!(w, "{ddl_text}")
                    .map_err(|e| SakiError::QueryFailed(format!("Write error: {e}")))?;
            }
            if include_data {
                let rows = stream_table_data(
                    &app_handle,
                    &state,
                    &conn_id,
                    &qualified,
                    &writer,
                    &cancel_flag,
                    total_rows,
                    None,
                )
                .await?;
                total_rows += rows;
            }
        }
        Ok(())
    }
    .await;

    state.export_cancel_flags.remove(&conn_id);

    match result {
        Ok(()) => {
            writer
                .lock()
                .unwrap()
                .flush()
                .map_err(|e| format!("Flush error: {e}"))?;
            emit_progress(&app_handle, total_rows, None, "complete");
            info!(rows = total_rows, tables = targets.len(), file_path = %file_path, "database SQL export complete");
            Ok(total_rows)
        }
        Err(SakiError::Cancelled) => {
            let _ = writer.lock().unwrap().flush();
            warn!(file_path = %file_path, "database SQL export cancelled");
            emit_progress(&app_handle, total_rows, None, "cancelled");
            Err("Export cancelled".to_string())
        }
        Err(e) => {
            error!(file_path = %file_path, error = %e, "database SQL export failed");
            emit_progress(&app_handle, total_rows, None, "error");
            Err(e.to_string())
        }
    }
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
