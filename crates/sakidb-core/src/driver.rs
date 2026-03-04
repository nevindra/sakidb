use async_trait::async_trait;

use crate::error::Result;
use crate::types::*;

#[async_trait]
pub trait DatabaseDriver: Send + Sync {
    async fn connect(&self, config: &ConnectionConfig) -> Result<ConnectionId>;
    async fn disconnect(&self, conn_id: &ConnectionId) -> Result<()>;
    async fn execute(&self, conn_id: &ConnectionId, sql: &str) -> Result<QueryResult>;
    async fn execute_multi(&self, conn_id: &ConnectionId, sql: &str) -> Result<MultiQueryResult>;
    async fn execute_paged(
        &self,
        conn_id: &ConnectionId,
        sql: &str,
        page: usize,
        page_size: usize,
    ) -> Result<PagedResult>;
    async fn list_databases(&self, conn_id: &ConnectionId) -> Result<Vec<DatabaseInfo>>;
    async fn list_schemas(&self, conn_id: &ConnectionId) -> Result<Vec<SchemaInfo>>;
    async fn list_tables(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
    ) -> Result<Vec<TableInfo>>;
    async fn list_columns(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
        table: &str,
    ) -> Result<Vec<ColumnInfo>>;
    async fn list_views(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
    ) -> Result<Vec<ViewInfo>>;
    async fn list_materialized_views(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
    ) -> Result<Vec<MaterializedViewInfo>>;
    async fn list_functions(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
    ) -> Result<Vec<FunctionInfo>>;
    async fn list_sequences(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
    ) -> Result<Vec<SequenceInfo>>;
    async fn list_indexes(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
    ) -> Result<Vec<IndexInfo>>;
    async fn list_foreign_tables(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
    ) -> Result<Vec<ForeignTableInfo>>;
    async fn list_triggers(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
        table: &str,
    ) -> Result<Vec<TriggerInfo>>;
    async fn list_foreign_keys(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
        table: &str,
    ) -> Result<Vec<ForeignKeyInfo>>;
    async fn list_check_constraints(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
        table: &str,
    ) -> Result<Vec<CheckConstraintInfo>>;
    async fn list_unique_constraints(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
        table: &str,
    ) -> Result<Vec<UniqueConstraintInfo>>;
    async fn get_partition_info(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
        table: &str,
    ) -> Result<Option<PartitionInfo>>;
    async fn get_create_table_sql(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
        table: &str,
    ) -> Result<String>;
    async fn execute_batch(&self, conn_id: &ConnectionId, sql: &str) -> Result<()>;
    async fn get_erd_data(&self, conn_id: &ConnectionId, schema: &str) -> Result<ErdData>;
    async fn test_connection(&self, config: &ConnectionConfig) -> Result<()>;
    async fn cancel_query(&self, conn_id: &ConnectionId) -> Result<()>;
}
