use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;
use oracle::Connection as OracleConnection;
use oracle::sql_type::OracleType;
use dashmap::DashMap;
use sakidb_core::{
    driver::{rows_to_columnar, paged_to_columnar},
    error::{Result, SakiError},
    types::{
        ConnectionId, QueryResult, MultiQueryResult, MultiColumnarResult,
        PagedResult, PagedColumnarResult, ColumnDef, CellValue, ExportBatchFn,
    },
};
use tracing::{info, warn, debug};

pub struct OracleExecutor {
    pub(crate) connections: Arc<DashMap<ConnectionId, Arc<RwLock<OracleConnection>>>>,
}

impl OracleExecutor {
    pub fn new(connections: Arc<DashMap<ConnectionId, Arc<RwLock<OracleConnection>>>>) -> Self {
        Self { connections }
    }

    pub(crate) fn get_connection(&self, conn_id: &ConnectionId) -> Result<Arc<RwLock<OracleConnection>>> {
        self.connections
            .get(conn_id)
            .map(|entry| entry.clone())
            .ok_or_else(|| SakiError::ConnectionNotFound(conn_id.0.to_string()))
    }

    pub(crate) fn convert_oracle_type_to_string(oracle_type: &OracleType) -> String {
        match oracle_type {
            OracleType::Varchar2(_) => "VARCHAR2".to_string(),
            OracleType::NVarchar2(_) => "NVARCHAR2".to_string(),
            OracleType::Number(_, _) => "NUMBER".to_string(),
            OracleType::Date => "DATE".to_string(),
            OracleType::Timestamp(_) => "TIMESTAMP".to_string(),
            OracleType::Char(_) => "CHAR".to_string(),
            OracleType::NChar(_) => "NCHAR".to_string(),
            OracleType::Raw(_) => "RAW".to_string(),
            OracleType::Float(_) => "BINARY_FLOAT".to_string(),
            OracleType::BinaryDouble => "BINARY_DOUBLE".to_string(),
            _ => format!("{:?}", oracle_type),
        }
    }

    fn row_value_to_cell(row: &oracle::Row, idx: usize) -> CellValue {
        // Try to get as various types, falling back to string
        if let Ok(None::<String>) = row.get::<_, Option<String>>(idx) {
            return CellValue::Null;
        }
        // Try i64
        if let Ok(Some(v)) = row.get::<_, Option<i64>>(idx) {
            return CellValue::Int(v);
        }
        // Try f64
        if let Ok(Some(v)) = row.get::<_, Option<f64>>(idx) {
            let f = v;
            if f.fract() == 0.0 && f >= i64::MIN as f64 && f <= i64::MAX as f64 {
                return CellValue::Int(f as i64);
            }
            return CellValue::Float(f);
        }
        // Try bool
        if let Ok(Some(v)) = row.get::<_, Option<bool>>(idx) {
            return CellValue::Bool(v);
        }
        // Fall back to string
        match row.get::<_, Option<String>>(idx) {
            Ok(Some(s)) => CellValue::Text(s.into_boxed_str()),
            Ok(None) => CellValue::Null,
            Err(_) => CellValue::Null,
        }
    }

    pub(crate) fn is_query(sql: &str) -> bool {
        let upper = sql.trim_start().to_uppercase();
        upper.starts_with("SELECT")
            || upper.starts_with("WITH")
            || upper.starts_with("EXPLAIN")
    }

    async fn execute_single(&self, conn: Arc<RwLock<OracleConnection>>, sql: String) -> Result<QueryResult> {
        let start = std::time::Instant::now();
        let is_query = Self::is_query(&sql);

        if is_query {
            let result = tokio::task::spawn_blocking(move || {
                let conn = conn.blocking_read();
                let result_set = conn.query(&sql, &[])
                    .map_err(|e| SakiError::QueryFailed(format!("Oracle query failed: {}", e)))?;

                let mut columns: Vec<ColumnDef> = Vec::new();
                let mut cells: Vec<CellValue> = Vec::new();

                // Collect column info from first row or column_info
                let col_info: Vec<oracle::ColumnInfo> = result_set.column_info().to_vec();
                for ci in &col_info {
                    columns.push(ColumnDef {
                        name: ci.name().to_string(),
                        data_type: Self::convert_oracle_type_to_string(ci.oracle_type()),
                    });
                }

                for row_result in result_set {
                    let row = row_result.map_err(|e| SakiError::QueryFailed(format!("Row fetch error: {}", e)))?;
                    for i in 0..columns.len() {
                        cells.push(Self::row_value_to_cell(&row, i));
                    }
                }

                let num_cols = columns.len();
                let row_count = if num_cols > 0 { (cells.len() / num_cols) as u64 } else { 0 };

                Ok::<QueryResult, SakiError>(QueryResult {
                    columns,
                    cells,
                    row_count,
                    execution_time_ms: 0,
                    truncated: false,
                })
            })
            .await
            .map_err(|e| SakiError::QueryFailed(format!("Query task failed: {}", e)))??;

            let elapsed = start.elapsed().as_millis() as u64;
            Ok(QueryResult { execution_time_ms: elapsed, ..result })
        } else {
            // DML / DDL
            let rows_affected = tokio::task::spawn_blocking(move || {
                let conn = conn.blocking_read();
                let stmt = conn.execute(&sql, &[])
                    .map_err(|e| SakiError::QueryFailed(format!("Oracle execute failed: {}", e)))?;
                let affected = stmt.row_count()
                    .map_err(|e| SakiError::QueryFailed(format!("Failed to get row count: {}", e)))?;
                Ok::<u64, SakiError>(affected)
            })
            .await
            .map_err(|e| SakiError::QueryFailed(format!("Execute task failed: {}", e)))??;

            let elapsed = start.elapsed().as_millis() as u64;
            Ok(QueryResult {
                columns: vec![],
                cells: vec![],
                row_count: rows_affected,
                execution_time_ms: elapsed,
                truncated: false,
            })
        }
    }

    pub async fn execute(&self, conn_id: &ConnectionId, sql: &str) -> Result<QueryResult> {
        debug!("Executing Oracle query: {}", sql);
        let conn = self.get_connection(conn_id)?;
        self.execute_single(conn, sql.to_string()).await
    }

    pub async fn execute_multi(&self, conn_id: &ConnectionId, sql: &str) -> Result<MultiQueryResult> {
        let conn = self.get_connection(conn_id)?;
        let start = std::time::Instant::now();
        // Split by semicolons (simple split; respects basic quoting not handled here)
        let statements: Vec<&str> = sql
            .split(';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        let mut results = Vec::new();
        for stmt in statements {
            let result = self.execute_single(conn.clone(), stmt.to_string()).await?;
            results.push(result);
        }

        Ok(MultiQueryResult {
            results,
            total_execution_time_ms: start.elapsed().as_millis() as u64,
        })
    }

    pub async fn execute_multi_columnar(
        &self,
        conn_id: &ConnectionId,
        sql: &str,
    ) -> Result<MultiColumnarResult> {
        let multi = self.execute_multi(conn_id, sql).await?;
        Ok(rows_to_columnar(multi))
    }

    pub async fn execute_paged(
        &self,
        conn_id: &ConnectionId,
        sql: &str,
        page: usize,
        page_size: usize,
    ) -> Result<PagedResult> {
        let conn = self.get_connection(conn_id)?;
        let offset = page * page_size;
        let paged_sql = format!(
            "SELECT * FROM (SELECT saki_inner__.*, ROWNUM saki_rnum__ FROM ({}) saki_inner__ WHERE ROWNUM <= {}) WHERE saki_rnum__ > {}",
            sql, offset + page_size, offset
        );

        let result = self.execute_single(conn.clone(), paged_sql).await?;

        // Get total count
        let count_sql = format!("SELECT COUNT(*) FROM ({})", sql);
        let count_result = self.execute_single(conn, count_sql).await?;
        let total_rows = count_result.cells.first().and_then(|c| match c {
            CellValue::Int(i) => Some(*i),
            CellValue::Float(f) => Some(*f as i64),
            _ => None,
        });

        Ok(PagedResult {
            columns: result.columns,
            cells: result.cells,
            row_count: result.row_count,
            page,
            page_size,
            total_rows_estimate: total_rows,
            execution_time_ms: result.execution_time_ms,
        })
    }

    pub async fn execute_paged_columnar(
        &self,
        conn_id: &ConnectionId,
        sql: &str,
        page: usize,
        page_size: usize,
    ) -> Result<PagedColumnarResult> {
        let paged = self.execute_paged(conn_id, sql, page, page_size).await?;
        Ok(paged_to_columnar(paged))
    }

    pub async fn execute_batch(&self, conn_id: &ConnectionId, sql: &str) -> Result<()> {
        let conn = self.get_connection(conn_id)?;
        let statements: Vec<String> = sql
            .split(';')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        for stmt in statements {
            tokio::task::spawn_blocking({
                let conn = conn.clone();
                let stmt = stmt.clone();
                move || {
                    let conn = conn.blocking_read();
                    conn.execute(&stmt, &[])
                        .map_err(|e| SakiError::QueryFailed(format!("Batch execute failed: {}", e)))?;
                    Ok::<(), SakiError>(())
                }
            })
            .await
            .map_err(|e| SakiError::QueryFailed(format!("Batch task failed: {}", e)))??;
        }

        Ok(())
    }

    pub async fn cancel_query(&self, _conn_id: &ConnectionId) -> Result<()> {
        warn!("Oracle query cancellation not implemented");
        Ok(())
    }

    pub async fn export_stream(
        &self,
        conn_id: &ConnectionId,
        sql: &str,
        batch_size: usize,
        cancelled: &AtomicBool,
        on_batch: &ExportBatchFn,
    ) -> Result<u64> {
        let conn = self.get_connection(conn_id)?;
        let mut offset = 0usize;
        let mut total_rows = 0u64;

        loop {
            if cancelled.load(Ordering::Relaxed) {
                info!("Export cancelled by user");
                break;
            }

            let page_sql = format!(
                "SELECT * FROM (SELECT saki_inner__.*, ROWNUM saki_rnum__ FROM ({}) saki_inner__ WHERE ROWNUM <= {}) WHERE saki_rnum__ > {}",
                sql, offset + batch_size, offset
            );

            let result = self.execute_single(conn.clone(), page_sql).await?;

            if result.row_count == 0 {
                break;
            }

            let num_cols = result.columns.len();
            if num_cols > 0 {
                for row_start in (0..result.cells.len()).step_by(num_cols) {
                    let row_end = (row_start + num_cols).min(result.cells.len());
                    on_batch(&result.columns, &result.cells[row_start..row_end], total_rows + (row_start / num_cols) as u64)?;
                }
            }

            total_rows += result.row_count;
            offset += batch_size;
        }

        Ok(total_rows)
    }
}
