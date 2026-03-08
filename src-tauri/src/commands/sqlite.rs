use sakidb_core::types::ConnectionId;
use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub async fn vacuum_database(
    state: State<'_, AppState>,
    conn_id: String,
) -> Result<(), String> {
    let conn_id = ConnectionId(
        uuid::Uuid::parse_str(&conn_id).map_err(|e| e.to_string())?,
    );

    let sql = state.registry.sql_for(&conn_id).map_err(|e| e.to_string())?;
    sql.execute_batch(&conn_id, "VACUUM")
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn check_integrity(
    state: State<'_, AppState>,
    conn_id: String,
) -> Result<Vec<String>, String> {
    let conn_id = ConnectionId(
        uuid::Uuid::parse_str(&conn_id).map_err(|e| e.to_string())?,
    );

    let sql = state.registry.sql_for(&conn_id).map_err(|e| e.to_string())?;
    let result = sql
        .execute(&conn_id, "PRAGMA integrity_check")
        .await
        .map_err(|e| e.to_string())?;

    // Extract text values from the result cells
    let messages: Vec<String> = result
        .cells
        .into_iter()
        .filter_map(|cell| match cell {
            sakidb_core::types::CellValue::Text(s) => Some(s.to_string()),
            _ => None,
        })
        .collect();

    Ok(messages)
}
