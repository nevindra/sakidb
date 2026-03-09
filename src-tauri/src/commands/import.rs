use std::sync::atomic::Ordering;

use tauri::{Emitter, State};
use tracing::info;

use sakidb_core::types::{ConnectionId, RestoreOptions, RestoreProgress};

use crate::state::AppState;

fn parse_conn_id(id: &str) -> Result<ConnectionId, String> {
    Ok(ConnectionId(
        uuid::Uuid::parse_str(id).map_err(|e| e.to_string())?,
    ))
}

#[tauri::command]
pub async fn restore_from_sql(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    active_connection_id: String,
    file_path: String,
    schema: Option<String>,
    continue_on_error: bool,
) -> Result<(), String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    info!(file_path = %file_path, schema = ?schema, continue_on_error, "restore requested");

    // Reset cancel flag
    state.restore_cancelled.store(false, Ordering::Relaxed);

    let options = RestoreOptions {
        schema,
        continue_on_error,
    };

    let app_handle = app.clone();
    let on_progress = Box::new(move |progress: &RestoreProgress| {
        let _ = app_handle.emit("restore-progress", progress);
    });

    state
        .registry
        .restorer_for(&conn_id)
        .map_err(|e| e.to_string())?
        .restore(
            &conn_id,
            &file_path,
            &options,
            &state.restore_cancelled,
            on_progress,
        )
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn cancel_restore(state: State<'_, AppState>) -> Result<(), String> {
    info!("restore cancel requested");
    state.restore_cancelled.store(true, Ordering::Relaxed);
    Ok(())
}
