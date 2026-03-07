use std::collections::HashMap;

use tauri::State;

use sakidb_core::types::*;

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
    state.registry.introspector_for(&conn_id).map_err(|e| e.to_string())?.list_databases(&conn_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_schemas(
    state: State<'_, AppState>,
    active_connection_id: String,
) -> Result<Vec<SchemaInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.registry.introspector_for(&conn_id).map_err(|e| e.to_string())?.list_schemas(&conn_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_tables(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
) -> Result<Vec<TableInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.registry.introspector_for(&conn_id).map_err(|e| e.to_string())?.list_tables(&conn_id, &schema).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_columns(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
    table: String,
) -> Result<Vec<ColumnInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.registry.introspector_for(&conn_id).map_err(|e| e.to_string())?.list_columns(&conn_id, &schema, &table).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_views(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
) -> Result<Vec<ViewInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.registry.introspector_for(&conn_id).map_err(|e| e.to_string())?.list_views(&conn_id, &schema).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_materialized_views(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
) -> Result<Vec<MaterializedViewInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.registry.introspector_for(&conn_id).map_err(|e| e.to_string())?.list_materialized_views(&conn_id, &schema).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_functions(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
) -> Result<Vec<FunctionInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.registry.introspector_for(&conn_id).map_err(|e| e.to_string())?.list_functions(&conn_id, &schema).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_sequences(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
) -> Result<Vec<SequenceInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.registry.introspector_for(&conn_id).map_err(|e| e.to_string())?.list_sequences(&conn_id, &schema).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_indexes(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
) -> Result<Vec<IndexInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.registry.introspector_for(&conn_id).map_err(|e| e.to_string())?.list_indexes(&conn_id, &schema).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_foreign_tables(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
) -> Result<Vec<ForeignTableInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.registry.introspector_for(&conn_id).map_err(|e| e.to_string())?.list_foreign_tables(&conn_id, &schema).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_triggers(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
    table: String,
) -> Result<Vec<TriggerInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.registry.introspector_for(&conn_id).map_err(|e| e.to_string())?.list_triggers(&conn_id, &schema, &table).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_foreign_keys(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
    table: String,
) -> Result<Vec<ForeignKeyInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.registry.introspector_for(&conn_id).map_err(|e| e.to_string())?.list_foreign_keys(&conn_id, &schema, &table).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_check_constraints(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
    table: String,
) -> Result<Vec<CheckConstraintInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.registry.introspector_for(&conn_id).map_err(|e| e.to_string())?.list_check_constraints(&conn_id, &schema, &table).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_unique_constraints(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
    table: String,
) -> Result<Vec<UniqueConstraintInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.registry.introspector_for(&conn_id).map_err(|e| e.to_string())?.list_unique_constraints(&conn_id, &schema, &table).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_partition_info(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
    table: String,
) -> Result<Option<PartitionInfo>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.registry.introspector_for(&conn_id).map_err(|e| e.to_string())?.get_partition_info(&conn_id, &schema, &table).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_erd_data(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
) -> Result<ErdData, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.registry.introspector_for(&conn_id).map_err(|e| e.to_string())?.get_erd_data(&conn_id, &schema).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_create_table_sql(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
    table: String,
) -> Result<String, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.registry.introspector_for(&conn_id).map_err(|e| e.to_string())?.get_create_table_sql(&conn_id, &schema, &table).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_schema_completion_data(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
) -> Result<HashMap<String, Vec<String>>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.registry.introspector_for(&conn_id).map_err(|e| e.to_string())?.get_schema_completion_data(&conn_id, &schema).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_completion_bundle(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
) -> Result<CompletionBundle, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.registry.introspector_for(&conn_id).map_err(|e| e.to_string())?.get_completion_bundle(&conn_id, &schema).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_table_columns_for_completion(
    state: State<'_, AppState>,
    active_connection_id: String,
    schema: String,
    table: String,
) -> Result<Vec<CompletionColumn>, String> {
    let conn_id = parse_conn_id(&active_connection_id)?;
    state.registry.introspector_for(&conn_id).map_err(|e| e.to_string())?.get_table_columns_for_completion(&conn_id, &schema, &table).await.map_err(|e| e.to_string())
}
