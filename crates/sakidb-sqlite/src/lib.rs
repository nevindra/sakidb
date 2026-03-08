pub mod connection;
pub mod executor;
pub mod introspect;
pub mod restore;

#[cfg(test)]
mod executor_test;

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;

use async_trait::async_trait;
use tracing::info;

use sakidb_core::types::*;
use sakidb_core::{Driver, Exporter, Introspector, Restorer, Result, SqlDriver};

use crate::connection::ConnectionManager;

pub struct SqliteDriver {
    manager: ConnectionManager,
}

impl SqliteDriver {
    pub fn new() -> Self {
        Self {
            manager: ConnectionManager::new(),
        }
    }

    /// Run VACUUM on a connected SQLite database.
    pub async fn vacuum(&self, conn_id: &ConnectionId) -> Result<()> {
        let cid = conn_id.0;
        self.manager
            .with_conn(conn_id, move |conn| {
                info!(conn_id = %cid, "running VACUUM");
                conn.execute_batch("VACUUM")
                    .map_err(|e| sakidb_core::SakiError::QueryFailed(e.to_string()))
            })
            .await
    }

    /// Run PRAGMA integrity_check on a connected SQLite database.
    pub async fn check_integrity(&self, conn_id: &ConnectionId) -> Result<Vec<String>> {
        let cid = conn_id.0;
        self.manager
            .with_conn(conn_id, move |conn| {
                info!(conn_id = %cid, "running integrity check");
                let mut stmt = conn
                    .prepare("PRAGMA integrity_check")
                    .map_err(|e| sakidb_core::SakiError::QueryFailed(e.to_string()))?;
                let results: Vec<String> = stmt
                    .query_map([], |row| row.get::<_, String>(0))
                    .map_err(|e| sakidb_core::SakiError::QueryFailed(e.to_string()))?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| sakidb_core::SakiError::QueryFailed(e.to_string()))?;
                Ok(results)
            })
            .await
    }
}

#[async_trait]
impl Driver for SqliteDriver {
    fn engine_type(&self) -> EngineType {
        EngineType::Sqlite
    }

    fn capabilities(&self) -> EngineCapabilities {
        EngineCapabilities {
            sql: true,
            introspection: true,
            export: true,
            restore: true,
            key_value: false,
            document: false,
            schemas: false,
            tables: true,
            views: true,
            materialized_views: false,
            functions: false,
            sequences: false,
            indexes: true,
            triggers: true,
            partitions: false,
            foreign_tables: false,
            explain: true,
            multi_database: false,
        }
    }

    async fn connect(&self, config: &ConnectionConfig) -> Result<ConnectionId> {
        let file_path = config.database.clone();
        self.manager.connect(&file_path)
    }

    async fn disconnect(&self, conn_id: &ConnectionId) -> Result<()> {
        self.manager.disconnect(conn_id)
    }

    async fn test_connection(&self, config: &ConnectionConfig) -> Result<()> {
        let file_path = config.database.clone();
        ConnectionManager::test_connection(&file_path)
    }
}

#[async_trait]
impl SqlDriver for SqliteDriver {
    async fn execute(&self, conn_id: &ConnectionId, sql: &str) -> Result<QueryResult> {
        let sql = sql.to_string();
        self.manager
            .with_conn(conn_id, move |conn| executor::execute_query(conn, &sql))
            .await
    }

    async fn execute_multi(&self, conn_id: &ConnectionId, sql: &str) -> Result<MultiQueryResult> {
        let sql = sql.to_string();
        self.manager
            .with_conn(conn_id, move |conn| executor::execute_multi(conn, &sql))
            .await
    }

    async fn execute_multi_columnar(
        &self,
        conn_id: &ConnectionId,
        sql: &str,
    ) -> Result<MultiColumnarResult> {
        let sql = sql.to_string();
        self.manager
            .with_conn(conn_id, move |conn| {
                executor::execute_multi_columnar(conn, &sql)
            })
            .await
    }

    async fn execute_paged(
        &self,
        conn_id: &ConnectionId,
        sql: &str,
        page: usize,
        page_size: usize,
    ) -> Result<PagedResult> {
        let sql = sql.to_string();
        self.manager
            .with_conn(conn_id, move |conn| {
                executor::execute_paged(conn, &sql, page, page_size)
            })
            .await
    }

    async fn execute_batch(&self, conn_id: &ConnectionId, sql: &str) -> Result<()> {
        let sql = sql.to_string();
        self.manager
            .with_conn(conn_id, move |conn| executor::execute_batch(conn, &sql))
            .await
    }

    async fn cancel_query(&self, conn_id: &ConnectionId) -> Result<()> {
        self.manager.interrupt(conn_id)
    }
}

#[async_trait]
impl Introspector for SqliteDriver {
    async fn list_databases(&self, _conn_id: &ConnectionId) -> Result<Vec<DatabaseInfo>> {
        Ok(vec![])
    }

    async fn list_schemas(&self, _conn_id: &ConnectionId) -> Result<Vec<SchemaInfo>> {
        Ok(vec![])
    }

    async fn list_tables(
        &self,
        conn_id: &ConnectionId,
        _schema: &str,
    ) -> Result<Vec<TableInfo>> {
        self.manager
            .with_conn(conn_id, introspect::list_tables)
            .await
    }

    async fn list_columns(
        &self,
        conn_id: &ConnectionId,
        _schema: &str,
        table: &str,
    ) -> Result<Vec<ColumnInfo>> {
        let table = table.to_string();
        self.manager
            .with_conn(conn_id, move |conn| introspect::list_columns(conn, &table))
            .await
    }

    async fn list_views(
        &self,
        conn_id: &ConnectionId,
        _schema: &str,
    ) -> Result<Vec<ViewInfo>> {
        self.manager
            .with_conn(conn_id, introspect::list_views)
            .await
    }

    async fn list_materialized_views(
        &self,
        _conn_id: &ConnectionId,
        _schema: &str,
    ) -> Result<Vec<MaterializedViewInfo>> {
        Ok(vec![])
    }

    async fn list_functions(
        &self,
        _conn_id: &ConnectionId,
        _schema: &str,
    ) -> Result<Vec<FunctionInfo>> {
        Ok(vec![])
    }

    async fn list_sequences(
        &self,
        _conn_id: &ConnectionId,
        _schema: &str,
    ) -> Result<Vec<SequenceInfo>> {
        Ok(vec![])
    }

    async fn list_indexes(
        &self,
        conn_id: &ConnectionId,
        _schema: &str,
    ) -> Result<Vec<IndexInfo>> {
        self.manager
            .with_conn(conn_id, introspect::list_all_indexes)
            .await
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
        _schema: &str,
        table: &str,
    ) -> Result<Vec<TriggerInfo>> {
        let table = table.to_string();
        self.manager
            .with_conn(conn_id, move |conn| {
                introspect::list_triggers(conn, &table)
            })
            .await
    }

    async fn list_foreign_keys(
        &self,
        conn_id: &ConnectionId,
        _schema: &str,
        table: &str,
    ) -> Result<Vec<ForeignKeyInfo>> {
        let table = table.to_string();
        self.manager
            .with_conn(conn_id, move |conn| {
                introspect::list_foreign_keys(conn, &table)
            })
            .await
    }

    async fn list_check_constraints(
        &self,
        conn_id: &ConnectionId,
        _schema: &str,
        table: &str,
    ) -> Result<Vec<CheckConstraintInfo>> {
        let table = table.to_string();
        self.manager
            .with_conn(conn_id, move |conn| {
                introspect::list_check_constraints(conn, &table)
            })
            .await
    }

    async fn list_unique_constraints(
        &self,
        conn_id: &ConnectionId,
        _schema: &str,
        table: &str,
    ) -> Result<Vec<UniqueConstraintInfo>> {
        let table = table.to_string();
        self.manager
            .with_conn(conn_id, move |conn| {
                introspect::list_unique_constraints(conn, &table)
            })
            .await
    }

    async fn get_partition_info(
        &self,
        _conn_id: &ConnectionId,
        _schema: &str,
        _table: &str,
    ) -> Result<Option<PartitionInfo>> {
        Ok(None)
    }

    async fn get_create_table_sql(
        &self,
        conn_id: &ConnectionId,
        _schema: &str,
        table: &str,
    ) -> Result<String> {
        let table = table.to_string();
        self.manager
            .with_conn(conn_id, move |conn| {
                introspect::get_create_table_sql(conn, &table)
            })
            .await
    }

    async fn get_erd_data(&self, conn_id: &ConnectionId, _schema: &str) -> Result<ErdData> {
        self.manager
            .with_conn(conn_id, introspect::get_erd_data)
            .await
    }

    async fn get_schema_completion_data(
        &self,
        conn_id: &ConnectionId,
        _schema: &str,
    ) -> Result<HashMap<String, Vec<String>>> {
        self.manager
            .with_conn(conn_id, introspect::get_schema_completion_data)
            .await
    }

    async fn get_completion_bundle(
        &self,
        conn_id: &ConnectionId,
        _schema: &str,
    ) -> Result<CompletionBundle> {
        self.manager
            .with_conn(conn_id, introspect::get_completion_bundle)
            .await
    }

    async fn get_table_columns_for_completion(
        &self,
        conn_id: &ConnectionId,
        _schema: &str,
        table: &str,
    ) -> Result<Vec<CompletionColumn>> {
        let table = table.to_string();
        self.manager
            .with_conn(conn_id, move |conn| {
                introspect::get_table_columns_for_completion(conn, &table)
            })
            .await
    }
}

#[async_trait]
impl Exporter for SqliteDriver {
    async fn export_stream(
        &self,
        conn_id: &ConnectionId,
        sql: &str,
        batch_size: usize,
        cancelled: &AtomicBool,
        on_batch: &ExportBatchFn,
    ) -> Result<u64> {
        let conn = self.manager.get_conn(conn_id)?;
        tokio::task::block_in_place(|| {
            let conn = conn.lock().map_err(|e| {
                sakidb_core::SakiError::QueryFailed(format!("lock poisoned: {e}"))
            })?;
            let mut callback =
                |cols: &[ColumnDef], cells: &[CellValue], total: u64| on_batch(cols, cells, total);
            executor::execute_export(&conn, sql, batch_size, &mut callback, cancelled)
        })
    }
}

#[async_trait]
impl Restorer for SqliteDriver {
    async fn restore(
        &self,
        conn_id: &ConnectionId,
        file_path: &str,
        options: &RestoreOptions,
        cancelled: &AtomicBool,
        on_progress: Box<dyn Fn(RestoreProgress) + Send + Sync>,
    ) -> Result<RestoreProgress> {
        let conn = self.manager.get_conn(conn_id)?;
        let continue_on_error = options.continue_on_error;
        let file_path = file_path.to_string();
        tokio::task::block_in_place(|| {
            let conn = conn.lock().map_err(|e| {
                sakidb_core::SakiError::QueryFailed(format!("lock poisoned: {e}"))
            })?;
            restore::restore_from_sql(&conn, &file_path, continue_on_error, cancelled, &*on_progress)
        })
    }
}
