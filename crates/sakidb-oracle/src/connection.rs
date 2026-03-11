use async_trait::async_trait;
use dashmap::DashMap;
use oracle::{Connection as OracleConnection, Connector};
use sakidb_core::{
    driver::{Driver, SqlDriver, Introspector, Exporter, Restorer, SqlFormatter},
    error::{Result, SakiError},
    types::{
        EngineType, EngineCapabilities, ConnectionConfig, ConnectionId,
        QueryResult, MultiQueryResult, MultiColumnarResult, PagedResult, PagedColumnarResult,
        DatabaseInfo, SchemaInfo, TableInfo, ColumnInfo, ViewInfo, MaterializedViewInfo,
        FunctionInfo, SequenceInfo, IndexInfo, ForeignTableInfo, TriggerInfo,
        ForeignKeyInfo, CheckConstraintInfo, UniqueConstraintInfo, PartitionInfo,
        ErdData, CompletionBundle, CompletionColumn, RestoreOptions, RestoreProgress,
        ExportBatchFn, ColumnDef, CellValue, DdlContext,
    },
};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::{executor::OracleExecutor, introspect::OracleIntrospector, restore::OracleRestorer, formatter::OracleFormatter};
use crate::instantclient::ensure_instantclient;

pub struct OracleDriver {
    connections: Arc<DashMap<ConnectionId, Arc<RwLock<OracleConnection>>>>,
}

impl OracleDriver {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(DashMap::new()),
        }
    }

    async fn setup_instantclient(&self) -> Result<()> {
        ensure_instantclient().await
    }

    pub(crate) fn build_connection_string(config: &ConnectionConfig) -> String {
        if let Some(tns) = config.options.get("tns") {
            tns.clone()
        } else {
            format!("{}:{}/{}", config.host, config.port, config.database)
        }
    }
}

impl Default for OracleDriver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Driver for OracleDriver {
    fn engine_type(&self) -> EngineType {
        EngineType::Oracle
    }

    fn capabilities(&self) -> EngineCapabilities {
        EngineCapabilities {
            sql: true,
            introspection: true,
            export: true,
            restore: true,
            key_value: false,
            document: false,
            schemas: true,
            tables: true,
            views: true,
            materialized_views: true,
            functions: true,
            sequences: true,
            indexes: true,
            triggers: true,
            partitions: true,
            foreign_tables: false,
            explain: true,
            multi_database: false,
        }
    }

    async fn connect(&self, config: &ConnectionConfig) -> Result<ConnectionId> {
        self.setup_instantclient().await?;

        let connection_string = Self::build_connection_string(config);
        info!("Connecting to Oracle: {}", connection_string);

        let connector = Connector::new(
            config.username.clone(),
            config.password.clone(),
            connection_string.clone(),
        );

        let connection = tokio::task::spawn_blocking(move || {
            connector.connect()
        })
        .await
        .map_err(|e| SakiError::ConnectionFailed(format!("Connection task failed: {}", e)))?
        .map_err(|e| SakiError::ConnectionFailed(format!("Failed to connect to Oracle: {}", e)))?;

        let conn_id = ConnectionId::new();
        self.connections.insert(conn_id, Arc::new(RwLock::new(connection)));

        info!("Successfully connected to Oracle: {}", conn_id.0);
        Ok(conn_id)
    }

    async fn disconnect(&self, conn_id: &ConnectionId) -> Result<()> {
        if let Some((_, conn)) = self.connections.remove(conn_id) {
            info!("Disconnecting Oracle connection: {}", conn_id.0);
            // Try to close; errors are logged but not fatal — connection already removed from registry
            let conn_guard = conn.read().await;
            if let Err(e) = conn_guard.close() {
                warn!("Error closing Oracle connection: {}", e);
            }
            drop(conn_guard);
            info!("Disconnected Oracle connection: {}", conn_id.0);
        } else {
            return Err(SakiError::ConnectionNotFound(conn_id.0.to_string()));
        }
        Ok(())
    }

    async fn test_connection(&self, config: &ConnectionConfig) -> Result<()> {
        self.setup_instantclient().await?;

        let connection_string = Self::build_connection_string(config);
        info!("Testing Oracle connection: {}", connection_string);

        let username = config.username.clone();
        let password = config.password.clone();

        let connection = tokio::task::spawn_blocking(move || {
            let conn = oracle::Connection::connect(&username, &password, &connection_string)
                .map_err(|e| SakiError::ConnectionFailed(format!("Failed to connect to Oracle: {}", e)))?;
            // Run a simple test query
            conn.query("SELECT 1 FROM DUAL", &[])
                .map_err(|e| SakiError::ConnectionFailed(format!("Test query failed: {}", e)))?;
            conn.close()
                .map_err(|e| SakiError::ConnectionFailed(format!("Failed to close test connection: {}", e)))?;
            Ok::<(), SakiError>(())
        })
        .await
        .map_err(|e| SakiError::ConnectionFailed(format!("Connection test task failed: {}", e)))??;

        let _ = connection;
        info!("Oracle connection test successful");
        Ok(())
    }
}

#[async_trait]
impl SqlDriver for OracleDriver {
    async fn execute(&self, conn_id: &ConnectionId, sql: &str) -> Result<QueryResult> {
        let executor = OracleExecutor::new(self.connections.clone());
        executor.execute(conn_id, sql).await
    }

    async fn execute_multi(&self, conn_id: &ConnectionId, sql: &str) -> Result<MultiQueryResult> {
        let executor = OracleExecutor::new(self.connections.clone());
        executor.execute_multi(conn_id, sql).await
    }

    async fn execute_multi_columnar(
        &self,
        conn_id: &ConnectionId,
        sql: &str,
    ) -> Result<MultiColumnarResult> {
        let executor = OracleExecutor::new(self.connections.clone());
        executor.execute_multi_columnar(conn_id, sql).await
    }

    async fn execute_paged(
        &self,
        conn_id: &ConnectionId,
        sql: &str,
        page: usize,
        page_size: usize,
    ) -> Result<PagedResult> {
        let executor = OracleExecutor::new(self.connections.clone());
        executor.execute_paged(conn_id, sql, page, page_size).await
    }

    async fn execute_paged_columnar(
        &self,
        conn_id: &ConnectionId,
        sql: &str,
        page: usize,
        page_size: usize,
    ) -> Result<PagedColumnarResult> {
        let executor = OracleExecutor::new(self.connections.clone());
        executor.execute_paged_columnar(conn_id, sql, page, page_size).await
    }

    async fn execute_batch(&self, conn_id: &ConnectionId, sql: &str) -> Result<()> {
        let executor = OracleExecutor::new(self.connections.clone());
        executor.execute_batch(conn_id, sql).await
    }

    async fn cancel_query(&self, conn_id: &ConnectionId) -> Result<()> {
        let executor = OracleExecutor::new(self.connections.clone());
        executor.cancel_query(conn_id).await
    }
}

#[async_trait]
impl Introspector for OracleDriver {
    async fn list_databases(&self, conn_id: &ConnectionId) -> Result<Vec<DatabaseInfo>> {
        let introspector = OracleIntrospector::new(self.connections.clone());
        introspector.list_databases(conn_id).await
    }

    async fn list_schemas(&self, conn_id: &ConnectionId) -> Result<Vec<SchemaInfo>> {
        let introspector = OracleIntrospector::new(self.connections.clone());
        introspector.list_schemas(conn_id).await
    }

    async fn list_tables(&self, conn_id: &ConnectionId, schema: &str) -> Result<Vec<TableInfo>> {
        let introspector = OracleIntrospector::new(self.connections.clone());
        introspector.list_tables(conn_id, schema).await
    }

    async fn list_columns(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
        table: &str,
    ) -> Result<Vec<ColumnInfo>> {
        let introspector = OracleIntrospector::new(self.connections.clone());
        introspector.list_columns(conn_id, schema, table).await
    }

    async fn list_views(&self, conn_id: &ConnectionId, schema: &str) -> Result<Vec<ViewInfo>> {
        let introspector = OracleIntrospector::new(self.connections.clone());
        introspector.list_views(conn_id, schema).await
    }

    async fn list_materialized_views(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
    ) -> Result<Vec<MaterializedViewInfo>> {
        let introspector = OracleIntrospector::new(self.connections.clone());
        introspector.list_materialized_views(conn_id, schema).await
    }

    async fn list_functions(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
    ) -> Result<Vec<FunctionInfo>> {
        let introspector = OracleIntrospector::new(self.connections.clone());
        introspector.list_functions(conn_id, schema).await
    }

    async fn list_sequences(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
    ) -> Result<Vec<SequenceInfo>> {
        let introspector = OracleIntrospector::new(self.connections.clone());
        introspector.list_sequences(conn_id, schema).await
    }

    async fn list_indexes(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
    ) -> Result<Vec<IndexInfo>> {
        let introspector = OracleIntrospector::new(self.connections.clone());
        introspector.list_indexes(conn_id, schema).await
    }

    async fn list_foreign_tables(
        &self,
        _conn_id: &ConnectionId,
        _schema: &str,
    ) -> Result<Vec<ForeignTableInfo>> {
        Ok(vec![])
    }

    async fn list_triggers(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
        table: &str,
    ) -> Result<Vec<TriggerInfo>> {
        let introspector = OracleIntrospector::new(self.connections.clone());
        introspector.list_triggers(conn_id, schema, table).await
    }

    async fn list_foreign_keys(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
        table: &str,
    ) -> Result<Vec<ForeignKeyInfo>> {
        let introspector = OracleIntrospector::new(self.connections.clone());
        introspector.list_foreign_keys(conn_id, schema, table).await
    }

    async fn list_check_constraints(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
        table: &str,
    ) -> Result<Vec<CheckConstraintInfo>> {
        let introspector = OracleIntrospector::new(self.connections.clone());
        introspector.list_check_constraints(conn_id, schema, table).await
    }

    async fn list_unique_constraints(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
        table: &str,
    ) -> Result<Vec<UniqueConstraintInfo>> {
        let introspector = OracleIntrospector::new(self.connections.clone());
        introspector.list_unique_constraints(conn_id, schema, table).await
    }

    async fn get_partition_info(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
        table: &str,
    ) -> Result<Option<PartitionInfo>> {
        let introspector = OracleIntrospector::new(self.connections.clone());
        introspector.get_partition_info(conn_id, schema, table).await
    }

    async fn get_create_table_sql(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
        table: &str,
    ) -> Result<String> {
        let introspector = OracleIntrospector::new(self.connections.clone());
        introspector.get_create_table_sql(conn_id, schema, table).await
    }

    async fn get_erd_data(&self, conn_id: &ConnectionId, schema: &str) -> Result<ErdData> {
        let introspector = OracleIntrospector::new(self.connections.clone());
        introspector.get_erd_data(conn_id, schema).await
    }

    async fn get_schema_completion_data(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
    ) -> Result<HashMap<String, Vec<String>>> {
        let introspector = OracleIntrospector::new(self.connections.clone());
        introspector.get_schema_completion_data(conn_id, schema).await
    }

    async fn get_completion_bundle(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
    ) -> Result<CompletionBundle> {
        let introspector = OracleIntrospector::new(self.connections.clone());
        introspector.get_completion_bundle(conn_id, schema).await
    }

    async fn get_table_columns_for_completion(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
        table: &str,
    ) -> Result<Vec<CompletionColumn>> {
        let introspector = OracleIntrospector::new(self.connections.clone());
        introspector.get_table_columns_for_completion(conn_id, schema, table).await
    }
}

#[async_trait]
impl Exporter for OracleDriver {
    async fn export_stream(
        &self,
        conn_id: &ConnectionId,
        sql: &str,
        batch_size: usize,
        cancelled: &AtomicBool,
        on_batch: &ExportBatchFn,
    ) -> Result<u64> {
        let executor = OracleExecutor::new(self.connections.clone());
        executor.export_stream(conn_id, sql, batch_size, cancelled, on_batch).await
    }
}

#[async_trait]
impl Restorer for OracleDriver {
    async fn restore(
        &self,
        conn_id: &ConnectionId,
        file_path: &str,
        options: &RestoreOptions,
        cancelled: &AtomicBool,
        on_progress: Box<dyn for<'a> Fn(&'a RestoreProgress) + Send + Sync>,
    ) -> Result<RestoreProgress> {
        let restorer = OracleRestorer::new(self.connections.clone());
        restorer.restore(conn_id, file_path, options, cancelled, on_progress).await
    }
}

impl SqlFormatter for OracleDriver {
    fn format_ddl(&self, ctx: &DdlContext<'_>) -> Option<String> {
        OracleFormatter.format_ddl(ctx)
    }

    fn format_data_header(&self, columns: &[ColumnDef], qualified_table: &str) -> Option<String> {
        OracleFormatter.format_data_header(columns, qualified_table)
    }

    fn format_data_row(
        &self,
        columns: &[ColumnDef],
        cells: &[CellValue],
        qualified_table: &str,
        buf: &mut String,
    ) {
        OracleFormatter.format_data_row(columns, cells, qualified_table, buf);
    }

    fn format_data_footer(&self) -> Option<String> {
        OracleFormatter.format_data_footer()
    }
}
