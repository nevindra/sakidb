use tauri::State;

use sakidb_store::models::{QueryHistoryEntry, SavedQuery};

use crate::state::AppState;

#[tauri::command]
pub async fn save_query(
    state: State<'_, AppState>,
    name: String,
    sql: String,
    connection_id: Option<String>,
    database_name: Option<String>,
) -> Result<SavedQuery, String> {
    let store = state.store.lock().await;
    store
        .save_query(&name, &sql, connection_id.as_deref(), database_name.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_saved_queries(
    state: State<'_, AppState>,
) -> Result<Vec<SavedQuery>, String> {
    let store = state.store.lock().await;
    store.list_saved_queries().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_saved_query(
    state: State<'_, AppState>,
    id: String,
    name: Option<String>,
    sql: Option<String>,
) -> Result<SavedQuery, String> {
    let store = state.store.lock().await;
    store
        .update_saved_query(&id, name.as_deref(), sql.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_saved_query(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let store = state.store.lock().await;
    store.delete_saved_query(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_query_history(
    state: State<'_, AppState>,
    sql: String,
    connection_id: Option<String>,
    database_name: Option<String>,
    execution_time_ms: Option<i64>,
    row_count: Option<i64>,
) -> Result<QueryHistoryEntry, String> {
    let store = state.store.lock().await;
    store
        .add_query_history(
            &sql,
            connection_id.as_deref(),
            database_name.as_deref(),
            execution_time_ms,
            row_count,
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_query_history(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<QueryHistoryEntry>, String> {
    let store = state.store.lock().await;
    store.list_query_history(limit).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_query_history(
    state: State<'_, AppState>,
) -> Result<(), String> {
    let store = state.store.lock().await;
    store.clear_query_history().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_from_history(
    state: State<'_, AppState>,
    history_id: String,
    name: String,
) -> Result<SavedQuery, String> {
    let store = state.store.lock().await;
    store
        .save_from_history(&history_id, &name)
        .map_err(|e| e.to_string())
}
