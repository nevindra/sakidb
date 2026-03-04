use std::collections::HashMap;

use tauri::State;

use sakidb_core::types::*;
use sakidb_core::DatabaseDriver;

use crate::state::AppState;

fn parse_conn_id(id: &str) -> Result<ConnectionId, String> {
    Ok(ConnectionId(
        uuid::Uuid::parse_str(id).map_err(|e| e.to_string())?,
    ))
}

#[tauri::command]
pub async fn list_databases(
    state: State<'_, AppState>,
    active_connection_id: String,
) -> Result<Vec<DatabaseInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.driver.list_databases(&conn_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_schemas(
    state: State<'_, AppState>,
    active_connection_id: String,
) -> Result<Vec<SchemaInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.driver.list_schemas(&conn_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_tables(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
) -> Result<Vec<TableInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.driver.list_tables(&conn_id, &schema).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_columns(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
    table: String,
) -> Result<Vec<ColumnInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.driver.list_columns(&conn_id, &schema, &table).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_views(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
) -> Result<Vec<ViewInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.driver.list_views(&conn_id, &schema).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_materialized_views(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
) -> Result<Vec<MaterializedViewInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.driver.list_materialized_views(&conn_id, &schema).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_functions(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
) -> Result<Vec<FunctionInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.driver.list_functions(&conn_id, &schema).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_sequences(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
) -> Result<Vec<SequenceInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.driver.list_sequences(&conn_id, &schema).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_indexes(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
) -> Result<Vec<IndexInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.driver.list_indexes(&conn_id, &schema).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_foreign_tables(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
) -> Result<Vec<ForeignTableInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.driver.list_foreign_tables(&conn_id, &schema).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_triggers(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
    table: String,
) -> Result<Vec<TriggerInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.driver.list_triggers(&conn_id, &schema, &table).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_foreign_keys(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
    table: String,
) -> Result<Vec<ForeignKeyInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.driver.list_foreign_keys(&conn_id, &schema, &table).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_check_constraints(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
    table: String,
) -> Result<Vec<CheckConstraintInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.driver.list_check_constraints(&conn_id, &schema, &table).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_unique_constraints(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
    table: String,
) -> Result<Vec<UniqueConstraintInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.driver.list_unique_constraints(&conn_id, &schema, &table).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_partition_info(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
    table: String,
) -> Result<Option<PartitionInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.driver.get_partition_info(&conn_id, &schema, &table).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_erd_data(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
) -> Result<ErdData, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.driver.get_erd_data(&conn_id, &schema).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_create_table_sql(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
    table: String,
) -> Result<String, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.driver.get_create_table_sql(&conn_id, &schema, &table).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_schema_completion_data(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
) -> Result<HashMap<String, Vec<String>>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    let pool = state.driver.get_pool(&conn_id).await.map_err(|e| e.to_string())?;
    sakidb_postgres::introspect::get_schema_completion_data(&pool, &schema)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_completion_bundle(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
) -> Result<CompletionBundle, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    let pool = state.driver.get_pool(&conn_id).await.map_err(|e| e.to_string())?;
    sakidb_postgres::introspect::get_completion_bundle(&pool, &schema)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_table_columns_for_completion(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
    table: String,
) -> Result<Vec<CompletionColumn>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    let pool = state.driver.get_pool(&conn_id).await.map_err(|e| e.to_string())?;
    sakidb_postgres::introspect::get_table_columns_for_completion(&pool, &schema, &table)
        .await
        .map_err(|e| e.to_string())
}
