use std::collections::HashMap;

use tauri::State;
use tracing::{error, info, warn};

use sakidb_core::types::{ConnectResult, ConnectionConfig, ConnectionId, EngineType, SslMode};
use sakidb_store::models::ConnectionInput;

use crate::state::AppState;

#[tauri::command]
pub async fn available_engines(
    state: State<'_, AppState>,
) -> Result<Vec<EngineType>, String> {
    Ok(state.registry.available_engines())
}

#[tauri::command]
pub async fn save_connection(
    state: State<'_, AppState>,
    input: ConnectionInput,
) -> Result<sakidb_store::models::SavedConnection, String> {
    let store = state.store.lock().await;
    store.save_connection(&input).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_connections(
    state: State<'_, AppState>,
) -> Result<Vec<sakidb_store::models::SavedConnection>, String> {
    let store = state.store.lock().await;
    store.list_connections().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_connection(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let store = state.store.lock().await;
    store.delete_connection(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_connection(
    state: State<'_, AppState>,
    id: String,
    input: ConnectionInput,
) -> Result<(), String> {
    let store = state.store.lock().await;
    store.update_connection(&id, &input).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn test_connection(
    state: State<'_, AppState>,
    input: ConnectionInput,
    id: Option<String>,
) -> Result<(), String> {
    // When editing an existing connection with no password entered,
    // look up the stored password so the test uses valid credentials.
    // If the lookup fails (e.g. unsaved connection), fall back to empty.
    let stored_password = if input.password.is_empty() {
        if let Some(ref conn_id) = id {
            let store = state.store.lock().await;
            store.get_connection(conn_id).ok().map(|c| c.password).unwrap_or_default()
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let config = input_to_config(&input, &stored_password);
    state
        .registry
        .driver_by_engine(&config.engine)
        .map_err(|e| e.to_string())?
        .test_connection(&config)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn connect_to_database(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<ConnectResult, String> {
    let store = state.store.lock().await;
    let saved = store.get_connection(&connection_id).map_err(|e| e.to_string())?;
    let config = saved_to_config(&saved, None);
    drop(store);

    let conn_id = state.registry.connect(&config).await.map_err(|e| {
        error!(connection_id = %connection_id, error = %e, "connect failed");
        e.to_string()
    })?;
    let capabilities = state.registry.capabilities_for(&conn_id).map_err(|e| e.to_string())?;
    info!(connection_id = %connection_id, runtime_id = %conn_id.0, "connected");
    Ok(ConnectResult { runtime_id: conn_id.0.to_string(), capabilities })
}

#[tauri::command]
pub async fn connect_to_database_as(
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
) -> Result<ConnectResult, String> {
    let store = state.store.lock().await;
    let saved = store.get_connection(&connection_id).map_err(|e| e.to_string())?;
    let config = saved_to_config(&saved, Some(database));
    drop(store);

    let conn_id = state.registry.connect(&config).await.map_err(|e| {
        error!(connection_id = %connection_id, database = %config.database, error = %e, "connect_as failed");
        e.to_string()
    })?;
    let capabilities = state.registry.capabilities_for(&conn_id).map_err(|e| e.to_string())?;
    info!(connection_id = %connection_id, database = %config.database, runtime_id = %conn_id.0, "connected as");
    Ok(ConnectResult { runtime_id: conn_id.0.to_string(), capabilities })
}

#[tauri::command]
pub async fn disconnect_from_database(
    state: State<'_, AppState>,
    active_connection_id: String,
) -> Result<(), String> {
    let conn_id = ConnectionId(
        uuid::Uuid::parse_str(&active_connection_id).map_err(|e| e.to_string())?,
    );

    state.registry.disconnect(&conn_id).await.map_err(|e| {
        warn!(conn_id = %active_connection_id, error = %e, "disconnect failed");
        e.to_string()
    })
}

#[tauri::command]
pub async fn drop_database(
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
) -> Result<(), String> {
    let store = state.store.lock().await;
    let saved = store.get_connection(&connection_id).map_err(|e| e.to_string())?;
    // Connect to 'postgres' maintenance database to issue DROP
    let config = saved_to_config(&saved, Some("postgres".to_string()));
    drop(store);

    let conn_id = state.registry.connect(&config).await.map_err(|e| e.to_string())?;

    // DROP DATABASE WITH (FORCE) terminates active sessions (PG 13+)
    info!(database = %database, "dropping database");
    let sql = format!("DROP DATABASE \"{}\" WITH (FORCE)", database.replace('"', "\"\""));
    let result = state
        .registry
        .sql_for(&conn_id)
        .map_err(|e| e.to_string())?
        .execute(&conn_id, &sql)
        .await;

    // Always disconnect the temp connection
    let _ = state.registry.disconnect(&conn_id).await;

    result.map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_last_connected(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let store = state.store.lock().await;
    store.update_last_connected(&id).map_err(|e| e.to_string())
}

fn input_to_config(input: &ConnectionInput, password: &str) -> ConnectionConfig {
    let engine: EngineType = input.engine.parse().unwrap_or(EngineType::Postgres);
    ConnectionConfig {
        engine,
        host: input.host.clone(),
        port: input.port,
        database: input.database.clone(),
        username: input.username.clone(),
        password: if password.is_empty() { input.password.clone() } else { password.to_string() },
        ssl_mode: parse_ssl_mode(&input.ssl_mode),
        options: HashMap::new(),
    }
}

fn saved_to_config(saved: &sakidb_store::models::SavedConnection, database: Option<String>) -> ConnectionConfig {
    let engine: EngineType = saved.engine.parse().unwrap_or(EngineType::Postgres);
    ConnectionConfig {
        engine,
        host: saved.host.clone(),
        port: saved.port,
        database: database.unwrap_or_else(|| saved.database.clone()),
        username: saved.username.clone(),
        password: saved.password.clone(),
        ssl_mode: parse_ssl_mode(&saved.ssl_mode),
        options: HashMap::new(),
    }
}

#[tauri::command]
pub async fn create_database(
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
) -> Result<(), String> {
    let store = state.store.lock().await;
    let saved = store.get_connection(&connection_id).map_err(|e| e.to_string())?;
    // Connect to 'postgres' maintenance database to issue CREATE
    let config = saved_to_config(&saved, Some("postgres".to_string()));
    drop(store);

    let conn_id = state.registry.connect(&config).await.map_err(|e| e.to_string())?;

    info!(database = %database, "creating database");
    let sql = format!("CREATE DATABASE \"{}\"", database.replace('"', "\"\""));
    let result = state
        .registry
        .sql_for(&conn_id)
        .map_err(|e| e.to_string())?
        .execute(&conn_id, &sql)
        .await;

    let _ = state.registry.disconnect(&conn_id).await;

    result.map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn rename_database(
    state: State<'_, AppState>,
    connection_id: String,
    old_name: String,
    new_name: String,
) -> Result<(), String> {
    let store = state.store.lock().await;
    let saved = store.get_connection(&connection_id).map_err(|e| e.to_string())?;
    // Connect to 'postgres' maintenance database to issue ALTER
    let config = saved_to_config(&saved, Some("postgres".to_string()));
    drop(store);

    let conn_id = state.registry.connect(&config).await.map_err(|e| e.to_string())?;

    info!(old_name = %old_name, new_name = %new_name, "renaming database");
    let sql = format!(
        "ALTER DATABASE \"{}\" RENAME TO \"{}\"",
        old_name.replace('"', "\"\""),
        new_name.replace('"', "\"\"")
    );
    let result = state
        .registry
        .sql_for(&conn_id)
        .map_err(|e| e.to_string())?
        .execute(&conn_id, &sql)
        .await;

    let _ = state.registry.disconnect(&conn_id).await;

    result.map(|_| ()).map_err(|e| e.to_string())
}

fn parse_ssl_mode(s: &str) -> SslMode {
    match s.to_lowercase().as_str() {
        "disable" => SslMode::Disable,
        "require" => SslMode::Require,
        _ => SslMode::Prefer,
    }
}
