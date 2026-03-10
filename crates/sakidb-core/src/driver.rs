use std::collections::HashMap;
use std::sync::atomic::AtomicBool;

use async_trait::async_trait;

use crate::error::Result;
use crate::types::*;

// ── Base trait — every engine implements this ──

#[async_trait]
pub trait Driver: Send + Sync {
    fn engine_type(&self) -> EngineType;
    fn capabilities(&self) -> EngineCapabilities;
    async fn connect(&self, config: &ConnectionConfig) -> Result<ConnectionId>;
    async fn disconnect(&self, conn_id: &ConnectionId) -> Result<()>;
    async fn test_connection(&self, config: &ConnectionConfig) -> Result<()>;
}

// ── SQL execution — Postgres, SQLite, DuckDB, ClickHouse ──

#[async_trait]
pub trait SqlDriver: Send + Sync {
    async fn execute(&self, conn_id: &ConnectionId, sql: &str) -> Result<QueryResult>;
    async fn execute_multi(&self, conn_id: &ConnectionId, sql: &str) -> Result<MultiQueryResult>;
    async fn execute_paged(
        &self,
        conn_id: &ConnectionId,
        sql: &str,
        page: usize,
        page_size: usize,
    ) -> Result<PagedResult>;
    async fn execute_batch(&self, conn_id: &ConnectionId, sql: &str) -> Result<()>;
    async fn cancel_query(&self, conn_id: &ConnectionId) -> Result<()>;

    /// Columnar results for large datasets.
    /// Default: converts execute_multi() output via rows_to_columnar().
    /// Drivers can override with native columnar path for better performance.
    async fn execute_multi_columnar(
        &self,
        conn_id: &ConnectionId,
        sql: &str,
    ) -> Result<MultiColumnarResult> {
        let result = self.execute_multi(conn_id, sql).await?;
        Ok(rows_to_columnar(result))
    }

    /// Columnar paged results for DataTab browsing.
    /// Default: converts execute_paged() output via paged_to_columnar().
    /// Drivers can override with native columnar streaming for better performance.
    async fn execute_paged_columnar(
        &self,
        conn_id: &ConnectionId,
        sql: &str,
        page: usize,
        page_size: usize,
    ) -> Result<PagedColumnarResult> {
        let result = self.execute_paged(conn_id, sql, page, page_size).await?;
        Ok(paged_to_columnar(result))
    }
}

// ── Schema introspection — relational databases ──

#[async_trait]
pub trait Introspector: Send + Sync {
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
    async fn get_erd_data(&self, conn_id: &ConnectionId, schema: &str) -> Result<ErdData>;

    // Completion support
    async fn get_schema_completion_data(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
    ) -> Result<HashMap<String, Vec<String>>>;
    async fn get_completion_bundle(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
    ) -> Result<CompletionBundle>;
    async fn get_table_columns_for_completion(
        &self,
        conn_id: &ConnectionId,
        schema: &str,
        table: &str,
    ) -> Result<Vec<CompletionColumn>>;
}

// ── Streaming data export ──

#[async_trait]
pub trait Exporter: Send + Sync {
    /// Stream query results in batches for memory-efficient export.
    async fn export_stream(
        &self,
        conn_id: &ConnectionId,
        sql: &str,
        batch_size: usize,
        cancelled: &AtomicBool,
        on_batch: &ExportBatchFn,
    ) -> Result<u64>;
}

// ── Data restore/import ──

#[async_trait]
pub trait Restorer: Send + Sync {
    async fn restore(
        &self,
        conn_id: &ConnectionId,
        file_path: &str,
        options: &RestoreOptions,
        cancelled: &AtomicBool,
        on_progress: Box<dyn for<'a> Fn(&'a RestoreProgress) + Send + Sync>,
    ) -> Result<RestoreProgress>;
}

// ── SQL export formatting — engine-specific DDL + data format ──

pub trait SqlFormatter: Send + Sync {
    /// Generate DDL for a table export (CREATE TABLE, indexes, triggers, constraints).
    fn format_ddl(&self, ctx: &DdlContext<'_>) -> Option<String>;

    /// Data section header. PG: "COPY ... FROM stdin;", SQLite: None.
    fn format_data_header(
        &self,
        columns: &[ColumnDef],
        qualified_table: &str,
    ) -> Option<String>;

    /// Format a single data row into buf.
    fn format_data_row(
        &self,
        columns: &[ColumnDef],
        cells: &[CellValue],
        qualified_table: &str,
        buf: &mut String,
    );

    /// Data section footer. PG: Some("\\."), SQLite: None.
    fn format_data_footer(&self) -> Option<String>;
}

// ── Key-value operations — Redis (future) ──

#[async_trait]
pub trait KeyValueDriver: Send + Sync {
    async fn get(&self, conn_id: &ConnectionId, key: &str) -> Result<Option<CellValue>>;
    async fn set(&self, conn_id: &ConnectionId, key: &str, value: &CellValue) -> Result<()>;
    async fn del(&self, conn_id: &ConnectionId, keys: &[&str]) -> Result<u64>;
    async fn keys(&self, conn_id: &ConnectionId, pattern: &str) -> Result<Vec<String>>;
    async fn scan(
        &self,
        conn_id: &ConnectionId,
        pattern: &str,
        cursor: u64,
        count: usize,
    ) -> Result<(u64, Vec<String>)>;
}

// ── Document operations — MongoDB (future) ──

#[async_trait]
pub trait DocumentDriver: Send + Sync {
    async fn find(
        &self,
        conn_id: &ConnectionId,
        collection: &str,
        filter: &str,
        limit: Option<usize>,
    ) -> Result<QueryResult>;
    async fn insert_one(
        &self,
        conn_id: &ConnectionId,
        collection: &str,
        document: &str,
    ) -> Result<String>;
    async fn list_collections(&self, conn_id: &ConnectionId) -> Result<Vec<String>>;
}

// ── Utility: convert row-based results to columnar format ──

/// Convert a PagedResult into a PagedColumnarResult by wrapping the single-result conversion.
pub fn paged_to_columnar(paged: PagedResult) -> PagedColumnarResult {
    let page = paged.page;
    let page_size = paged.page_size;
    let total_rows_estimate = paged.total_rows_estimate;

    let qr = QueryResult {
        columns: paged.columns,
        cells: paged.cells,
        row_count: paged.row_count,
        execution_time_ms: paged.execution_time_ms,
        truncated: false,
    };

    let multi = rows_to_columnar(MultiQueryResult {
        results: vec![qr],
        total_execution_time_ms: 0,
    });

    let result = multi.results.into_iter().next().unwrap();

    PagedColumnarResult {
        result,
        page,
        page_size,
        total_rows_estimate,
    }
}

pub fn rows_to_columnar(multi: MultiQueryResult) -> MultiColumnarResult {
    let total_execution_time_ms = multi.total_execution_time_ms;
    let results = multi
        .results
        .into_iter()
        .map(|qr| {
            let num_cols = qr.columns.len();
            let row_count = qr.row_count as usize;

            if num_cols == 0 {
                return ColumnarResult {
                    columns: qr.columns,
                    column_data: vec![],
                    row_count: qr.row_count,
                    execution_time_ms: qr.execution_time_ms,
                    truncated: qr.truncated,
                };
            }

            // Classify each column by type from first non-null cell
            let mut column_data: Vec<ColumnStorage> = Vec::with_capacity(num_cols);

            for col_idx in 0..num_cols {
                // Find first non-null cell to determine column type
                let mut col_type = None;
                for row_idx in 0..row_count {
                    match &qr.cells[row_idx * num_cols + col_idx] {
                        CellValue::Null => continue,
                        CellValue::Bool(_) => {
                            col_type = Some(1u8);
                            break;
                        }
                        CellValue::Int(_) | CellValue::Float(_) => {
                            col_type = Some(0u8);
                            break;
                        }
                        CellValue::Text(_)
                        | CellValue::Json(_)
                        | CellValue::Timestamp(_) => {
                            col_type = Some(2u8);
                            break;
                        }
                        CellValue::Bytes(_) => {
                            col_type = Some(3u8);
                            break;
                        }
                    }
                }

                match col_type.unwrap_or(2) {
                    0 => {
                        // Number column
                        let mut nulls = vec![0u8; row_count];
                        let mut values = vec![0.0f64; row_count];
                        for row_idx in 0..row_count {
                            match &qr.cells[row_idx * num_cols + col_idx] {
                                CellValue::Null => nulls[row_idx] = 1,
                                CellValue::Int(i) => values[row_idx] = *i as f64,
                                CellValue::Float(f) => values[row_idx] = *f,
                                _ => nulls[row_idx] = 1,
                            }
                        }
                        column_data.push(ColumnStorage::Number { nulls, values });
                    }
                    1 => {
                        // Bool column
                        let mut nulls = vec![0u8; row_count];
                        let mut values = vec![0u8; row_count];
                        for row_idx in 0..row_count {
                            match &qr.cells[row_idx * num_cols + col_idx] {
                                CellValue::Null => nulls[row_idx] = 1,
                                CellValue::Bool(b) => values[row_idx] = *b as u8,
                                _ => nulls[row_idx] = 1,
                            }
                        }
                        column_data.push(ColumnStorage::Bool { nulls, values });
                    }
                    2 => {
                        // Text column
                        let mut nulls = vec![0u8; row_count];
                        let mut offsets = Vec::with_capacity(row_count + 1);
                        let mut data = Vec::new();
                        offsets.push(0u32);
                        for (row_idx, null_flag) in nulls.iter_mut().enumerate() {
                            match &qr.cells[row_idx * num_cols + col_idx] {
                                CellValue::Null => {
                                    *null_flag = 1;
                                    offsets.push(data.len() as u32);
                                }
                                CellValue::Text(s)
                                | CellValue::Json(s)
                                | CellValue::Timestamp(s) => {
                                    data.extend_from_slice(s.as_bytes());
                                    offsets.push(data.len() as u32);
                                }
                                other => {
                                    let s = format!("{other:?}");
                                    data.extend_from_slice(s.as_bytes());
                                    offsets.push(data.len() as u32);
                                }
                            }
                        }
                        column_data
                            .push(ColumnStorage::Text { nulls, offsets, data });
                    }
                    3 => {
                        // Bytes column
                        let mut nulls = vec![0u8; row_count];
                        let mut offsets = Vec::with_capacity(row_count + 1);
                        let mut data = Vec::new();
                        offsets.push(0u32);
                        for (row_idx, null_flag) in nulls.iter_mut().enumerate() {
                            match &qr.cells[row_idx * num_cols + col_idx] {
                                CellValue::Null => {
                                    *null_flag = 1;
                                    offsets.push(data.len() as u32);
                                }
                                CellValue::Bytes(b) => {
                                    data.extend_from_slice(b);
                                    offsets.push(data.len() as u32);
                                }
                                _ => {
                                    *null_flag = 1;
                                    offsets.push(data.len() as u32);
                                }
                            }
                        }
                        column_data
                            .push(ColumnStorage::Bytes { nulls, offsets, data });
                    }
                    _ => unreachable!(),
                }
            }

            ColumnarResult {
                columns: qr.columns,
                column_data,
                row_count: qr.row_count,
                execution_time_ms: qr.execution_time_ms,
                truncated: qr.truncated,
            }
        })
        .collect();

    MultiColumnarResult {
        results,
        total_execution_time_ms,
    }
}
