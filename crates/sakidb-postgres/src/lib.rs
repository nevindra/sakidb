pub mod connection;
pub mod executor;
pub mod introspect;
pub mod restore;

#[cfg(test)]
mod executor_test;
#[cfg(test)]
mod restore_test;

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use tokio_postgres::{CancelToken, NoTls};
use tracing::{info, warn};

use sakidb_core::types::*;
use sakidb_core::{Driver, Exporter, Introspector, Restorer, Result, SqlDriver};

use crate::connection::ConnectionManager;

pub struct PostgresDriver {
    manager: ConnectionManager,
    cancel_tokens: Arc<DashMap<ConnectionId, CancelToken>>,
}

impl PostgresDriver {
    pub fn new() -> Self {
        Self {
            manager: ConnectionManager::new(),
            cancel_tokens: Arc::new(DashMap::new()),
        }
    }
}

#[async_trait]
impl Driver for PostgresDriver {
    fn engine_type(&self) -> EngineType {
        EngineType::Postgres
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
            foreign_tables: true,
            explain: true,
            multi_database: true,
        }
    }

    async fn connect(&self, config: &ConnectionConfig) -> Result<ConnectionId> {
        self.manager.connect(config).await
    }

    async fn disconnect(&self, conn_id: &ConnectionId) -> Result<()> {
        self.cancel_tokens.remove(conn_id);
        self.manager.disconnect(conn_id).await
    }

    async fn test_connection(&self, config: &ConnectionConfig) -> Result<()> {
        ConnectionManager::test_connection(config).await
    }
}

#[async_trait]
impl SqlDriver for PostgresDriver {
    async fn execute(&self, conn_id: &ConnectionId, sql: &str) -> Result<QueryResult> {
        let pool = self.manager.get_pool(conn_id).await?;
        executor::execute_query(&pool, sql, conn_id, &self.cancel_tokens).await
    }

    async fn execute_multi(&self, conn_id: &ConnectionId, sql: &str) -> Result<MultiQueryResult> {
        let pool = self.manager.get_pool(conn_id).await?;
        executor::execute_multi(&pool, sql, conn_id, &self.cancel_tokens).await
    }

    async fn execute_paged(
        &self,
        conn_id: &ConnectionId,
        sql: &str,
        page: usize,
        page_size: usize,
    ) -> Result<PagedResult> {
        let pool = self.manager.get_pool(conn_id).await?;
        executor::execute_paged(&pool, sql, page, page_size, conn_id, &self.cancel_tokens).await
    }

    async fn execute_batch(&self, conn_id: &ConnectionId, sql: &str) -> Result<()> {
        let pool = self.manager.get_pool(conn_id).await?;
        executor::execute_batch(&pool, sql).await
    }

    async fn cancel_query(&self, conn_id: &ConnectionId) -> Result<()> {
        let cancel_token = {
            let entry = self.cancel_tokens.get(conn_id);
            match entry {
                Some(token) => token.clone(),
                None => {
                    warn!(conn_id = %conn_id.0, "cancel_query: no running query");
                    return Ok(());
                }
            }
        };

        info!(conn_id = %conn_id.0, "cancelling query");
        cancel_token
            .cancel_query(NoTls)
            .await
            .map_err(|e| sakidb_core::SakiError::QueryFailed(e.to_string()))?;
        Ok(())
    }

    async fn execute_multi_columnar(
        &self,
        conn_id: &ConnectionId,
        sql: &str,
    ) -> Result<MultiColumnarResult> {
        let pool = self.manager.get_pool(conn_id).await?;
        executor::execute_multi_columnar(&pool, sql, conn_id, &self.cancel_tokens).await
    }

    async fn execute_paged_columnar(
        &self,
        conn_id: &ConnectionId,
        sql: &str,
        page: usize,
        page_size: usize,
    ) -> Result<PagedColumnarResult> {
        let pool = self.manager.get_pool(conn_id).await?;
        executor::execute_paged_columnar(&pool, sql, page, page_size, conn_id, &self.cancel_tokens)
            .await
    }
}

#[async_trait]
impl Introspector for PostgresDriver {
    async fn list_databases(&self, conn_id: &ConnectionId) -> Result<Vec<DatabaseInfo>> {
        let pool = self.manager.get_pool(conn_id).await?;
        introspect::list_databases(&pool).await
    }

    async fn list_schemas(&self, conn_id: &ConnectionId) -> Result<Vec<SchemaInfo>> {
        let pool = self.manager.get_pool(conn_id).await?;
        introspect::list_schemas(&pool).await
    }

    async fn list_tables(&self, conn_id: &ConnectionId, schema: &str) -> Result<Vec<TableInfo>> {
        let pool = self.manager.get_pool(conn_id).await?;
        introspect::list_tables(&pool, schema).await
    }

    async fn list_columns(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
        table: &str,
    ) -> Result<Vec<ColumnInfo>> {
        let pool = self.manager.get_pool(conn_id).await?;
        introspect::list_columns(&pool, schema, table).await
    }

    async fn list_views(&self, conn_id: &ConnectionId, schema: &str) -> Result<Vec<ViewInfo>> {
        let pool = self.manager.get_pool(conn_id).await?;
        introspect::list_views(&pool, schema).await
    }

    async fn list_materialized_views(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
    ) -> Result<Vec<MaterializedViewInfo>> {
        let pool = self.manager.get_pool(conn_id).await?;
        introspect::list_materialized_views(&pool, schema).await
    }

    async fn list_functions(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
    ) -> Result<Vec<FunctionInfo>> {
        let pool = self.manager.get_pool(conn_id).await?;
        introspect::list_functions(&pool, schema).await
    }

    async fn list_sequences(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
    ) -> Result<Vec<SequenceInfo>> {
        let pool = self.manager.get_pool(conn_id).await?;
        introspect::list_sequences(&pool, schema).await
    }

    async fn list_indexes(&self, conn_id: &ConnectionId, schema: &str) -> Result<Vec<IndexInfo>> {
        let pool = self.manager.get_pool(conn_id).await?;
        introspect::list_indexes(&pool, schema).await
    }

    async fn list_foreign_tables(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
    ) -> Result<Vec<ForeignTableInfo>> {
        let pool = self.manager.get_pool(conn_id).await?;
        introspect::list_foreign_tables(&pool, schema).await
    }

    async fn list_triggers(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
        table: &str,
    ) -> Result<Vec<TriggerInfo>> {
        let pool = self.manager.get_pool(conn_id).await?;
        introspect::list_triggers(&pool, schema, table).await
    }

    async fn list_foreign_keys(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
        table: &str,
    ) -> Result<Vec<ForeignKeyInfo>> {
        let pool = self.manager.get_pool(conn_id).await?;
        introspect::list_foreign_keys(&pool, schema, table).await
    }

    async fn list_check_constraints(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
        table: &str,
    ) -> Result<Vec<CheckConstraintInfo>> {
        let pool = self.manager.get_pool(conn_id).await?;
        introspect::list_check_constraints(&pool, schema, table).await
    }

    async fn list_unique_constraints(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
        table: &str,
    ) -> Result<Vec<UniqueConstraintInfo>> {
        let pool = self.manager.get_pool(conn_id).await?;
        introspect::list_unique_constraints(&pool, schema, table).await
    }

    async fn get_partition_info(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
        table: &str,
    ) -> Result<Option<PartitionInfo>> {
        let pool = self.manager.get_pool(conn_id).await?;
        introspect::get_partition_info(&pool, schema, table).await
    }

    async fn get_create_table_sql(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
        table: &str,
    ) -> Result<String> {
        let pool = self.manager.get_pool(conn_id).await?;
        introspect::get_create_table_sql(&pool, schema, table).await
    }

    async fn get_erd_data(&self, conn_id: &ConnectionId, schema: &str) -> Result<ErdData> {
        let pool = self.manager.get_pool(conn_id).await?;
        introspect::get_erd_data(&pool, schema).await
    }

    async fn get_schema_completion_data(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
    ) -> Result<HashMap<String, Vec<String>>> {
        let pool = self.manager.get_pool(conn_id).await?;
        introspect::get_schema_completion_data(&pool, schema).await
    }

    async fn get_completion_bundle(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
    ) -> Result<CompletionBundle> {
        let pool = self.manager.get_pool(conn_id).await?;
        introspect::get_completion_bundle(&pool, schema).await
    }

    async fn get_table_columns_for_completion(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
        table: &str,
    ) -> Result<Vec<CompletionColumn>> {
        let pool = self.manager.get_pool(conn_id).await?;
        introspect::get_table_columns_for_completion(&pool, schema, table).await
    }
}

#[async_trait]
impl Exporter for PostgresDriver {
    async fn export_stream(
        &self,
        conn_id: &ConnectionId,
        sql: &str,
        batch_size: usize,
        cancelled: &AtomicBool,
        on_batch: &ExportBatchFn,
    ) -> Result<u64> {
        let pool = self.manager.get_pool(conn_id).await?;
        let mut callback =
            |cols: &[ColumnDef], cells: &[CellValue], total: u64| on_batch(cols, cells, total);
        executor::execute_export_cursor(&pool, sql, batch_size, &mut callback, cancelled).await
    }
}

#[async_trait]
impl Restorer for PostgresDriver {
    async fn restore(
        &self,
        conn_id: &ConnectionId,
        file_path: &str,
        options: &RestoreOptions,
        cancelled: &AtomicBool,
        on_progress: Box<dyn for<'a> Fn(&'a RestoreProgress) + Send + Sync>,
    ) -> Result<RestoreProgress> {
        let pool = self.manager.get_pool(conn_id).await?;
        restore::restore_from_sql(
            &pool,
            file_path,
            options.schema.as_deref(),
            options.continue_on_error,
            cancelled,
            on_progress,
        )
        .await
    }
}
