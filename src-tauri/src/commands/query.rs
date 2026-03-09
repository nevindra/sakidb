use std::time::Instant;

use tauri::ipc::Response;
use tauri::State;
use tracing::{debug, info};

use sakidb_core::types::*;

use crate::state::AppState;

#[cfg(debug_assertions)]
fn rss_mb() -> f64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse::<f64>().ok())
        })
        .unwrap_or(0.0)
        / 1024.0
}

#[tauri::command]
pub async fn execute_query(
    state: State<'_, AppState>,
    active_connection_id: String,
    sql: String,
) -> Result<Response, String> {
    let conn_id = ConnectionId(
        uuid::Uuid::parse_str(&active_connection_id).map_err(|e| e.to_string())?,
    );
    let result = state
        .registry
        .sql_for(&conn_id)
        .map_err(|e| e.to_string())?
        .execute(&conn_id, &sql)
        .await
        .map_err(|e| e.to_string())?;
    let t0 = Instant::now();
    let bytes = rmp_serde::to_vec_named(&result).map_err(|e| e.to_string())?;
    debug!(
        elapsed_ms = result.execution_time_ms,
        encode_ms = t0.elapsed().as_millis() as u64,
        payload_bytes = bytes.len(),
        rows = result.row_count,
        "query IPC"
    );
    Ok(Response::new(bytes))
}

#[tauri::command]
pub async fn execute_query_multi(
    state: State<'_, AppState>,
    active_connection_id: String,
    sql: String,
) -> Result<Response, String> {
    let conn_id = ConnectionId(
        uuid::Uuid::parse_str(&active_connection_id).map_err(|e| e.to_string())?,
    );
    let result = state
        .registry
        .sql_for(&conn_id)
        .map_err(|e| e.to_string())?
        .execute_multi(&conn_id, &sql)
        .await
        .map_err(|e| e.to_string())?;
    let t0 = Instant::now();
    let bytes = rmp_serde::to_vec_named(&result).map_err(|e| e.to_string())?;
    debug!(
        elapsed_ms = result.total_execution_time_ms,
        statements = result.results.len(),
        encode_ms = t0.elapsed().as_millis() as u64,
        payload_bytes = bytes.len(),
        "multi-query IPC"
    );
    Ok(Response::new(bytes))
}

#[tauri::command]
pub async fn execute_query_multi_columnar(
    state: State<'_, AppState>,
    active_connection_id: String,
    sql: String,
) -> Result<Response, String> {
    let conn_id = ConnectionId(
        uuid::Uuid::parse_str(&active_connection_id).map_err(|e| e.to_string())?,
    );

    let start = Instant::now();
    let result = state
        .registry
        .sql_for(&conn_id)
        .map_err(|e| e.to_string())?
        .execute_multi_columnar(&conn_id, &sql)
        .await
        .map_err(|e| e.to_string())?;

    // Encode each result directly into the output buffer.
    // encode_into() consumes each ColumnarResult, freeing column storage
    // as each column is written — avoids holding storage + encoded copy simultaneously.
    let num_results = result.results.len();
    let total_exec_ms = result.total_execution_time_ms;
    #[cfg(debug_assertions)]
    debug!(rss_mb = format!("{:.0}", rss_mb()), "RSS before encode");
    let mut buf = Vec::new();
    buf.extend_from_slice(&(num_results as u32).to_le_bytes());
    buf.extend_from_slice(&total_exec_ms.to_le_bytes());
    for (_i, r) in result.results.into_iter().enumerate() {
        let _t0 = Instant::now();
        // Write placeholder for byte length, encode directly into buf
        let len_pos = buf.len();
        buf.extend_from_slice(&0u32.to_le_bytes());
        r.encode_into(&mut buf);
        let encoded_len = buf.len() - len_pos - 4;
        buf[len_pos..len_pos + 4].copy_from_slice(&(encoded_len as u32).to_le_bytes());
        #[cfg(debug_assertions)]
        debug!(
            index = _i,
            encoded_mb = format!("{:.1}", encoded_len as f64 / 1024.0 / 1024.0),
            encode_ms = _t0.elapsed().as_millis() as u64,
            rss_mb = format!("{:.0}", rss_mb()),
            "columnar encode"
        );
    }

    let total_ms = start.elapsed().as_millis() as u64;
    debug!(
        elapsed_ms = total_ms,
        statements = num_results,
        payload_mb = format!("{:.1}", buf.len() as f64 / 1024.0 / 1024.0),
        "columnar IPC complete"
    );

    Ok(Response::new(buf))
}

#[tauri::command]
pub async fn execute_query_paged(
    state: State<'_, AppState>,
    active_connection_id: String,
    sql: String,
    page: usize,
    page_size: usize,
) -> Result<Response, String> {
    let conn_id = ConnectionId(
        uuid::Uuid::parse_str(&active_connection_id).map_err(|e| e.to_string())?,
    );
    let result = state
        .registry
        .sql_for(&conn_id)
        .map_err(|e| e.to_string())?
        .execute_paged(&conn_id, &sql, page, page_size)
        .await
        .map_err(|e| e.to_string())?;
    let t0 = Instant::now();
    let bytes = rmp_serde::to_vec_named(&result).map_err(|e| e.to_string())?;
    debug!(
        elapsed_ms = result.execution_time_ms,
        encode_ms = t0.elapsed().as_millis() as u64,
        payload_bytes = bytes.len(),
        rows = result.row_count,
        page = result.page,
        page_size = result.page_size,
        total_estimate = ?result.total_rows_estimate,
        "paged query IPC"
    );
    Ok(Response::new(bytes))
}

#[tauri::command]
pub async fn execute_query_paged_columnar(
    state: State<'_, AppState>,
    active_connection_id: String,
    sql: String,
    page: usize,
    page_size: usize,
) -> Result<Response, String> {
    let conn_id = ConnectionId(
        uuid::Uuid::parse_str(&active_connection_id).map_err(|e| e.to_string())?,
    );

    let start = Instant::now();
    let result = state
        .registry
        .sql_for(&conn_id)
        .map_err(|e| e.to_string())?
        .execute_paged_columnar(&conn_id, &sql, page, page_size)
        .await
        .map_err(|e| e.to_string())?;

    let row_count = result.result.row_count;
    let total_estimate = result.total_rows_estimate;
    let result_page = result.page;

    let buf = result.encode();

    let total_ms = start.elapsed().as_millis() as u64;
    debug!(
        elapsed_ms = total_ms,
        rows = row_count,
        page = result_page,
        page_size,
        total_estimate = ?total_estimate,
        payload_kb = buf.len() / 1024,
        "paged columnar IPC complete"
    );

    Ok(Response::new(buf))
}

#[tauri::command]
pub async fn execute_batch(
    state: State<'_, AppState>,
    active_connection_id: String,
    sql: String,
) -> Result<(), String> {
    let conn_id = ConnectionId(
        uuid::Uuid::parse_str(&active_connection_id).map_err(|e| e.to_string())?,
    );
    let t0 = Instant::now();
    state
        .registry
        .sql_for(&conn_id)
        .map_err(|e| e.to_string())?
        .execute_batch(&conn_id, &sql)
        .await
        .map_err(|e| e.to_string())?;
    debug!(elapsed_ms = t0.elapsed().as_millis() as u64, "batch IPC");
    Ok(())
}

#[tauri::command]
pub async fn cancel_query(
    state: State<'_, AppState>,
    active_connection_id: String,
) -> Result<(), String> {
    info!(conn_id = %active_connection_id, "cancel query requested");
    let conn_id = ConnectionId(
        uuid::Uuid::parse_str(&active_connection_id).map_err(|e| e.to_string())?,
    );
    state
        .registry
        .sql_for(&conn_id)
        .map_err(|e| e.to_string())?
        .cancel_query(&conn_id)
        .await
        .map_err(|e| e.to_string())
}
