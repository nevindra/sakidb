use std::fmt::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use dashmap::DashMap;
use deadpool_postgres::Pool;
use futures_util::TryStreamExt;
use postgres_types::{FromSql, Type};
use tokio_postgres::types::ToSql;
use tokio_postgres::CancelToken;
use tracing::{debug, info};

use sakidb_core::sql::{SqlSplitOptions, split_sql_statements_with};
use sakidb_core::types::*;
use sakidb_core::SakiError;

/// Classify a PostgreSQL type into our columnar storage type.
/// Returns: 0=Number, 1=Bool, 2=Text, 3=Bytes
fn classify_pg_type(pg_type: &Type) -> u8 {
    match *pg_type {
        Type::BOOL => 1,
        Type::BYTEA => 3,
        Type::INT2 | Type::INT4 | Type::INT8 | Type::OID
        | Type::FLOAT4 | Type::FLOAT8 => 0,
        // NUMERIC is arbitrary-precision — store as Text to preserve exact values
        _ => 2, // everything else as text
    }
}

/// RAII guard that removes a cancel token from the DashMap when dropped.
/// Ensures cleanup even if the future is cancelled or panics.
struct CancelTokenGuard<'a> {
    map: &'a DashMap<ConnectionId, CancelToken>,
    id: ConnectionId,
}

impl<'a> CancelTokenGuard<'a> {
    fn new(
        map: &'a DashMap<ConnectionId, CancelToken>,
        id: ConnectionId,
        token: CancelToken,
    ) -> Self {
        map.insert(id, token);
        Self { map, id }
    }
}

impl Drop for CancelTokenGuard<'_> {
    fn drop(&mut self) {
        self.map.remove(&self.id);
    }
}

/// Maximum rows returned by unbounded execute_query before truncation.
const MAX_EXECUTE_ROWS: u64 = 100_000;

/// Zero-parse JSON extractor — reads raw JSON/JSONB bytes as a String
/// without parsing into serde_json::Value and re-serializing.
struct RawJsonString(String);

impl<'a> FromSql<'a> for RawJsonString {
    fn from_sql(
        ty: &Type,
        raw: &'a [u8],
    ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        // JSONB wire format: 1-byte version prefix + JSON text
        let bytes = if *ty == Type::JSONB && !raw.is_empty() {
            &raw[1..]
        } else {
            raw
        };
        Ok(RawJsonString(String::from_utf8(bytes.to_vec())?))
    }

    fn accepts(ty: &Type) -> bool {
        matches!(*ty, Type::JSON | Type::JSONB)
    }
}

/// Fallback extractor for extension/unknown types (e.g. pgvector).
/// Accepts any type and attempts a reasonable text representation.
struct FallbackText(String);

impl<'a> FromSql<'a> for FallbackText {
    fn from_sql(
        ty: &Type,
        raw: &'a [u8],
    ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        // pgvector binary: u16 ndim (BE) + u16 unused + ndim × f32 (BE)
        if ty.name() == "vector" && raw.len() >= 4 {
            let ndim = u16::from_be_bytes([raw[0], raw[1]]) as usize;
            let max_show = 3.min(ndim);
            let mut buf = String::with_capacity(64);
            buf.push('[');
            for i in 0..max_show {
                let off = 4 + i * 4;
                if off + 4 <= raw.len() {
                    if i > 0 { buf.push(','); }
                    let f = f32::from_be_bytes([raw[off], raw[off + 1], raw[off + 2], raw[off + 3]]);
                    let _ = write!(buf, "{f}");
                }
            }
            if ndim > max_show {
                let _ = write!(buf, ",... ({ndim} dims)");
            }
            buf.push(']');
            return Ok(FallbackText(buf));
        }
        // Try UTF-8 text for other extension types
        match std::str::from_utf8(raw) {
            Ok(s) => Ok(FallbackText(s.to_string())),
            Err(_) => Ok(FallbackText(format!("[binary: {} bytes]", raw.len()))),
        }
    }

    fn accepts(_ty: &Type) -> bool {
        true
    }
}

/// Extract a detailed error message from a tokio_postgres error.
/// The default Display for db errors is just "db error" — this extracts
/// severity, message, detail, and hint from the underlying DbError.
pub(crate) fn format_pg_error(e: &tokio_postgres::Error) -> String {
    if let Some(db_err) = e.as_db_error() {
        let mut msg = format!("{}: {}", db_err.severity(), db_err.message());
        if let Some(detail) = db_err.detail() {
            let _ = write!(msg, "\nDetail: {detail}");
        }
        if let Some(hint) = db_err.hint() {
            let _ = write!(msg, "\nHint: {hint}");
        }
        msg
    } else {
        e.to_string()
    }
}

/// Format a deadpool pool error, extracting pg details when available.
pub(crate) fn format_pool_error(e: &deadpool_postgres::PoolError) -> String {
    match e {
        deadpool_postgres::PoolError::Backend(pg_err) => format_pg_error(pg_err),
        other => other.to_string(),
    }
}

/// Push a single PostgreSQL cell value into the appropriate column storage.
fn push_columnar_value(
    row: &tokio_postgres::Row,
    col_idx: usize,
    pg_type: &Type,
    col_type: u8,
    storage: &mut ColumnStorage,
) {
    match (col_type, storage) {
        (0, ColumnStorage::Number { nulls, values }) => {
            let val: Option<f64> = match *pg_type {
                Type::INT2 => row.get::<_, Option<i16>>(col_idx).map(|v| v as f64),
                Type::INT4 => row.get::<_, Option<i32>>(col_idx).map(|v| v as f64),
                Type::INT8 => row.get::<_, Option<i64>>(col_idx).map(|v| v as f64),
                Type::OID => row.get::<_, Option<u32>>(col_idx).map(|v| v as f64),
                Type::FLOAT4 => row.get::<_, Option<f32>>(col_idx).map(|v| v as f64),
                Type::FLOAT8 => row.get::<_, Option<f64>>(col_idx),
                _ => None,
            };
            match val {
                Some(v) => {
                    nulls.push(0);
                    values.push(v);
                }
                None => {
                    nulls.push(1);
                    values.push(0.0);
                }
            }
        }
        (1, ColumnStorage::Bool { nulls, values }) => {
            match row.get::<_, Option<bool>>(col_idx) {
                Some(v) => {
                    nulls.push(0);
                    values.push(if v { 1 } else { 0 });
                }
                None => {
                    nulls.push(1);
                    values.push(0);
                }
            }
        }
        (2, ColumnStorage::Text { nulls, offsets, data }) => {
            let text = pg_value_to_text(row, col_idx, pg_type);
            match text {
                Some(s) => {
                    nulls.push(0);
                    data.extend_from_slice(s.as_bytes());
                    offsets.push(data.len() as u32);
                }
                None => {
                    nulls.push(1);
                    offsets.push(data.len() as u32);
                }
            }
        }
        (3, ColumnStorage::Bytes { nulls, offsets, data }) => {
            match row.get::<_, Option<Vec<u8>>>(col_idx) {
                Some(v) => {
                    nulls.push(0);
                    data.extend_from_slice(&v);
                    offsets.push(data.len() as u32);
                }
                None => {
                    nulls.push(1);
                    offsets.push(data.len() as u32);
                }
            }
        }
        _ => unreachable!(),
    }
}

/// Extract a PostgreSQL value as a UTF-8 string for the Text column storage.
fn pg_value_to_text(
    row: &tokio_postgres::Row,
    col_idx: usize,
    pg_type: &Type,
) -> Option<String> {
    match *pg_type {
        Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME => {
            row.get::<_, Option<String>>(col_idx)
        }
        Type::JSON | Type::JSONB => {
            row.get::<_, Option<RawJsonString>>(col_idx).map(|r| r.0)
        }
        Type::TIMESTAMP => {
            row.get::<_, Option<chrono::NaiveDateTime>>(col_idx)
                .map(|t| t.to_string())
        }
        Type::TIMESTAMPTZ => {
            row.get::<_, Option<chrono::DateTime<chrono::Utc>>>(col_idx)
                .map(|t| t.to_rfc3339())
        }
        Type::DATE => {
            row.get::<_, Option<chrono::NaiveDate>>(col_idx).map(|d| d.to_string())
        }
        Type::TIME | Type::TIMETZ => {
            row.get::<_, Option<chrono::NaiveTime>>(col_idx).map(|t| t.to_string())
        }
        Type::UUID => {
            row.get::<_, Option<uuid::Uuid>>(col_idx).map(|u| u.to_string())
        }
        _ => {
            // Fallback: try String, then FallbackText for extension types
            row.try_get::<_, Option<String>>(col_idx)
                .ok()
                .flatten()
                .or_else(|| {
                    row.try_get::<_, Option<FallbackText>>(col_idx)
                        .ok()
                        .flatten()
                        .map(|f| f.0)
                })
        }
    }
}

pub async fn execute_query_columnar(
    pool: &Pool,
    sql: &str,
    conn_id: &ConnectionId,
    cancel_tokens: &Arc<DashMap<ConnectionId, CancelToken>>,
) -> Result<ColumnarResult, SakiError> {
    let start = Instant::now();

    let client = pool
        .get()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;

    let _cancel_guard = CancelTokenGuard::new(cancel_tokens, *conn_id, client.cancel_token());

    let params: Vec<&(dyn ToSql + Sync)> = vec![];
    let stream = client
        .query_raw(sql, params)
        .await
        .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;
    futures_util::pin_mut!(stream);

    let mut columns: Vec<ColumnDef> = vec![];
    let mut col_types: Vec<u8> = vec![];
    let mut col_storages: Vec<ColumnStorage> = vec![];
    let mut row_count: u64 = 0;
    let mut truncated = false;

    while let Some(row) = stream
        .try_next()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?
    {
        if row_count == 0 {
            let cols = row.columns();
            columns = cols
                .iter()
                .map(|c| ColumnDef {
                    name: c.name().to_string(),
                    data_type: c.type_().name().to_string(),
                })
                .collect();
            col_types = cols.iter().map(|c| classify_pg_type(c.type_())).collect();

            let cap = 128usize;
            col_storages = col_types
                .iter()
                .map(|&t| match t {
                    0 => ColumnStorage::Number {
                        nulls: Vec::with_capacity(cap),
                        values: Vec::with_capacity(cap),
                    },
                    1 => ColumnStorage::Bool {
                        nulls: Vec::with_capacity(cap),
                        values: Vec::with_capacity(cap),
                    },
                    2 => ColumnStorage::Text {
                        nulls: Vec::with_capacity(cap),
                        offsets: vec![0],
                        data: Vec::with_capacity(cap * 32),
                    },
                    3 => ColumnStorage::Bytes {
                        nulls: Vec::with_capacity(cap),
                        offsets: vec![0],
                        data: Vec::with_capacity(cap * 64),
                    },
                    _ => unreachable!(),
                })
                .collect();
        }

        let cols_meta = row.columns();
        for i in 0..cols_meta.len() {
            push_columnar_value(&row, i, cols_meta[i].type_(), col_types[i], &mut col_storages[i]);
        }
        row_count += 1;

        if row_count >= MAX_EXECUTE_ROWS {
            truncated = true;
            break;
        }
    }

    let execution_time_ms = start.elapsed().as_millis() as u64;

    debug!(
        rows = row_count,
        cols = columns.len(),
        elapsed_ms = execution_time_ms,
        truncated,
        "columnar query complete"
    );

    Ok(ColumnarResult {
        columns,
        column_data: col_storages,
        row_count,
        execution_time_ms,
        truncated,
    })
}

pub async fn execute_multi_columnar(
    pool: &Pool,
    sql: &str,
    conn_id: &ConnectionId,
    cancel_tokens: &Arc<DashMap<ConnectionId, CancelToken>>,
) -> Result<MultiColumnarResult, SakiError> {
    let start = Instant::now();
    let statements = split_sql_statements(sql);
    debug!(statements = statements.len(), "executing multi columnar");
    let mut results = Vec::with_capacity(statements.len());

    for stmt in &statements {
        let trimmed = stmt.trim();
        if trimmed.is_empty() {
            continue;
        }
        results.push(
            execute_query_columnar(pool, trimmed, conn_id, cancel_tokens).await?,
        );
    }

    let elapsed = start.elapsed().as_millis() as u64;
    info!(statements = results.len(), elapsed_ms = elapsed, "multi columnar complete");

    Ok(MultiColumnarResult {
        total_execution_time_ms: elapsed,
        results,
    })
}

pub async fn execute_query(
    pool: &Pool,
    sql: &str,
    conn_id: &ConnectionId,
    cancel_tokens: &Arc<DashMap<ConnectionId, CancelToken>>,
) -> Result<QueryResult, SakiError> {
    let start = Instant::now();

    let client = pool
        .get()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;

    // RAII guard: cancel token is automatically cleaned up on drop (even on panic/cancel)
    let _cancel_guard = CancelTokenGuard::new(cancel_tokens, *conn_id, client.cancel_token());

    let params: Vec<&(dyn ToSql + Sync)> = vec![];
    let stream = client
        .query_raw(sql, params)
        .await
        .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;
    futures_util::pin_mut!(stream);

    let mut columns: Vec<ColumnDef> = vec![];
    let mut cells: Vec<CellValue> = Vec::new();
    let mut row_count: u64 = 0;
    let mut first_row_ms: u64 = 0;
    let mut truncated = false;

    while let Some(row) = stream
        .try_next()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?
    {
        if row_count == 0 {
            first_row_ms = start.elapsed().as_millis() as u64;
            columns = row
                .columns()
                .iter()
                .map(|c| ColumnDef {
                    name: c.name().to_string(),
                    data_type: c.type_().name().to_string(),
                })
                .collect();
            // Pre-allocate for 128 rows — a reasonable default for interactive queries.
            // Vec doubling handles growth beyond this without over-allocating for small results.
            cells.reserve(columns.len() * 128);
        }
        let cols = row.columns();
        for i in 0..cols.len() {
            cells.push(pg_value_to_cell(&row, i, cols[i].type_()));
        }
        row_count += 1;

        if row_count >= MAX_EXECUTE_ROWS {
            truncated = true;
            break;
        }
    }

    let execution_time_ms = start.elapsed().as_millis() as u64;
    debug!(
        first_row_ms,
        elapsed_ms = execution_time_ms,
        rows = row_count,
        cells = cells.len(),
        truncated,
        "query complete"
    );

    Ok(QueryResult {
        columns,
        cells,
        row_count,
        execution_time_ms,
        truncated,
    })
}

/// Fast row-count estimate using EXPLAIN's planner output.
/// Returns None if the estimate cannot be obtained.
async fn estimate_row_count(
    pool: &Pool,
    sql: &str,
) -> Option<i64> {
    let t0 = Instant::now();
    let client = pool.get().await.ok()?;
    let explain_sql = format!("EXPLAIN (FORMAT JSON) {sql}");
    let row = client.query_opt(&explain_sql, &[]).await.ok()??;
    let json: serde_json::Value = row.try_get(0).ok()?;
    // EXPLAIN JSON returns [{"Plan": {"Plan Rows": N, ...}}]
    let estimate = json.get(0)?
        .get("Plan")?
        .get("Plan Rows")?
        .as_f64()
        .map(|n| n as i64);
    debug!(elapsed_ms = t0.elapsed().as_millis() as u64, estimate = ?estimate, "row count estimate");
    estimate
}

pub async fn execute_paged(
    pool: &Pool,
    sql: &str,
    page: usize,
    page_size: usize,
    conn_id: &ConnectionId,
    cancel_tokens: &Arc<DashMap<ConnectionId, CancelToken>>,
) -> Result<PagedResult, SakiError> {
    let start = Instant::now();
    debug!(page, page_size, "executing paged query");
    let offset = page * page_size;

    let paged_sql = format!(
        "SELECT * FROM ({sql}) AS _paged_query LIMIT {page_size} OFFSET {offset}"
    );

    let data_client = pool
        .get()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;

    // RAII guard: cancel token is automatically cleaned up on drop (even on panic/cancel)
    let _cancel_guard = CancelTokenGuard::new(cancel_tokens, *conn_id, data_client.cancel_token());

    // Only run COUNT(*) on the first page — the frontend caches it for subsequent pages
    let run_count = page == 0;

    let data_future = async {
        let params: Vec<&(dyn ToSql + Sync)> = vec![];
        let stream = data_client
            .query_raw(&paged_sql, params)
            .await
            .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;
        futures_util::pin_mut!(stream);

        let mut columns: Vec<ColumnDef> = vec![];
        let mut cells: Vec<CellValue> = Vec::new();
        let mut row_count: u64 = 0;

        while let Some(row) = stream
            .try_next()
            .await
            .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?
        {
            if row_count == 0 {
                columns = row
                    .columns()
                    .iter()
                    .map(|c| ColumnDef {
                        name: c.name().to_string(),
                        data_type: c.type_().name().to_string(),
                    })
                    .collect();
                cells.reserve(columns.len() * page_size);
            }
            let cols = row.columns();
            for i in 0..cols.len() {
                cells.push(pg_value_to_cell(&row, i, cols[i].type_()));
            }
            row_count += 1;
        }

        Ok::<_, SakiError>((columns, cells, row_count))
    };

    let (total, data_result) = if run_count {
        let (est, data) = tokio::join!(
            estimate_row_count(pool, sql),
            data_future
        );
        (est, data)
    } else {
        let data = data_future.await;
        (None, data)
    };

    let (columns, cells, row_count) = data_result?;
    let execution_time_ms = start.elapsed().as_millis() as u64;

    debug!(
        page,
        page_size,
        rows = row_count,
        total_estimate = ?total,
        elapsed_ms = execution_time_ms,
        "paged query complete"
    );

    Ok(PagedResult {
        columns,
        cells,
        row_count,
        page,
        page_size,
        total_rows_estimate: total,
        execution_time_ms,
    })
}

pub async fn execute_paged_columnar(
    pool: &Pool,
    sql: &str,
    page: usize,
    page_size: usize,
    conn_id: &ConnectionId,
    cancel_tokens: &Arc<DashMap<ConnectionId, CancelToken>>,
) -> Result<PagedColumnarResult, SakiError> {
    let start = Instant::now();
    debug!(page, page_size, "executing paged columnar query");
    let offset = page * page_size;

    let paged_sql = format!(
        "SELECT * FROM ({sql}) AS _paged_query LIMIT {page_size} OFFSET {offset}"
    );

    let data_client = pool
        .get()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;

    let _cancel_guard = CancelTokenGuard::new(cancel_tokens, *conn_id, data_client.cancel_token());

    let run_count = page == 0;

    let data_future = async {
        let params: Vec<&(dyn ToSql + Sync)> = vec![];
        let stream = data_client
            .query_raw(&paged_sql, params)
            .await
            .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;
        futures_util::pin_mut!(stream);

        let mut columns: Vec<ColumnDef> = vec![];
        let mut col_types: Vec<u8> = vec![];
        let mut col_storages: Vec<ColumnStorage> = vec![];
        let mut row_count: u64 = 0;

        while let Some(row) = stream
            .try_next()
            .await
            .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?
        {
            if row_count == 0 {
                let cols = row.columns();
                columns = cols
                    .iter()
                    .map(|c| ColumnDef {
                        name: c.name().to_string(),
                        data_type: c.type_().name().to_string(),
                    })
                    .collect();
                col_types = cols.iter().map(|c| classify_pg_type(c.type_())).collect();

                let cap = page_size;
                col_storages = col_types
                    .iter()
                    .map(|&t| match t {
                        0 => ColumnStorage::Number {
                            nulls: Vec::with_capacity(cap),
                            values: Vec::with_capacity(cap),
                        },
                        1 => ColumnStorage::Bool {
                            nulls: Vec::with_capacity(cap),
                            values: Vec::with_capacity(cap),
                        },
                        2 => ColumnStorage::Text {
                            nulls: Vec::with_capacity(cap),
                            offsets: vec![0],
                            data: Vec::with_capacity(cap * 32),
                        },
                        3 => ColumnStorage::Bytes {
                            nulls: Vec::with_capacity(cap),
                            offsets: vec![0],
                            data: Vec::with_capacity(cap * 64),
                        },
                        _ => unreachable!(),
                    })
                    .collect();
            }

            let cols_meta = row.columns();
            for i in 0..cols_meta.len() {
                push_columnar_value(&row, i, cols_meta[i].type_(), col_types[i], &mut col_storages[i]);
            }
            row_count += 1;
        }

        Ok::<_, SakiError>((columns, col_storages, row_count))
    };

    let (total, data_result) = if run_count {
        let (est, data) = tokio::join!(
            estimate_row_count(pool, sql),
            data_future
        );
        (est, data)
    } else {
        let data = data_future.await;
        (None, data)
    };

    let (columns, col_storages, row_count) = data_result?;
    let execution_time_ms = start.elapsed().as_millis() as u64;

    debug!(
        page,
        page_size,
        rows = row_count,
        total_estimate = ?total,
        elapsed_ms = execution_time_ms,
        "paged columnar query complete"
    );

    Ok(PagedColumnarResult {
        result: ColumnarResult {
            columns,
            column_data: col_storages,
            row_count,
            execution_time_ms,
            truncated: false,
        },
        page,
        page_size,
        total_rows_estimate: total,
    })
}

pub async fn execute_batch(
    pool: &Pool,
    sql: &str,
) -> Result<(), SakiError> {
    let t0 = Instant::now();
    debug!("executing batch");
    let client = pool
        .get()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;
    client
        .batch_execute(sql)
        .await
        .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;
    debug!(elapsed_ms = t0.elapsed().as_millis() as u64, "batch complete");
    Ok(())
}

fn pg_value_to_cell(row: &tokio_postgres::Row, index: usize, pg_type: &Type) -> CellValue {
    match *pg_type {
        Type::BOOL => row
            .try_get::<_, Option<bool>>(index)
            .ok()
            .flatten()
            .map(CellValue::Bool)
            .unwrap_or(CellValue::Null),
        Type::INT2 => row
            .try_get::<_, Option<i16>>(index)
            .ok()
            .flatten()
            .map(|v| CellValue::Int(v as i64))
            .unwrap_or(CellValue::Null),
        Type::INT4 | Type::OID => row
            .try_get::<_, Option<i32>>(index)
            .ok()
            .flatten()
            .map(|v| CellValue::Int(v as i64))
            .unwrap_or(CellValue::Null),
        Type::INT8 => row
            .try_get::<_, Option<i64>>(index)
            .ok()
            .flatten()
            .map(CellValue::Int)
            .unwrap_or(CellValue::Null),
        Type::FLOAT4 => row
            .try_get::<_, Option<f32>>(index)
            .ok()
            .flatten()
            .map(|v| CellValue::Float(v as f64))
            .unwrap_or(CellValue::Null),
        Type::FLOAT8 => row
            .try_get::<_, Option<f64>>(index)
            .ok()
            .flatten()
            .map(CellValue::Float)
            .unwrap_or(CellValue::Null),
        // NUMERIC is arbitrary-precision — extracting as f64 silently loses precision.
        // Fall through to text extraction to preserve exact values.
        Type::NUMERIC => row
            .try_get::<_, Option<String>>(index)
            .ok()
            .flatten()
            .map(|v| CellValue::Text(v.into_boxed_str()))
            .unwrap_or(CellValue::Null),
        Type::JSON | Type::JSONB => row
            .try_get::<_, Option<RawJsonString>>(index)
            .ok()
            .flatten()
            .map(|v| CellValue::Json(v.0.into_boxed_str()))
            .unwrap_or(CellValue::Null),
        Type::BYTEA => row
            .try_get::<_, Option<Vec<u8>>>(index)
            .ok()
            .flatten()
            .map(|v| CellValue::Bytes(v.into_boxed_slice()))
            .unwrap_or(CellValue::Null),
        Type::TIMESTAMP => row
            .try_get::<_, Option<chrono::NaiveDateTime>>(index)
            .ok()
            .flatten()
            .map(|v| CellValue::Timestamp(v.to_string().into_boxed_str()))
            .unwrap_or(CellValue::Null),
        Type::TIMESTAMPTZ => row
            .try_get::<_, Option<chrono::DateTime<chrono::Utc>>>(index)
            .ok()
            .flatten()
            .map(|v| CellValue::Timestamp(v.to_rfc3339().into_boxed_str()))
            .unwrap_or(CellValue::Null),
        Type::DATE => row
            .try_get::<_, Option<chrono::NaiveDate>>(index)
            .ok()
            .flatten()
            .map(|v| CellValue::Timestamp(v.to_string().into_boxed_str()))
            .unwrap_or(CellValue::Null),
        Type::TIME | Type::TIMETZ => row
            .try_get::<_, Option<chrono::NaiveTime>>(index)
            .ok()
            .flatten()
            .map(|v| CellValue::Timestamp(v.to_string().into_boxed_str()))
            .unwrap_or(CellValue::Null),
        Type::UUID => row
            .try_get::<_, Option<uuid::Uuid>>(index)
            .ok()
            .flatten()
            .map(|v| CellValue::Text(v.to_string().into_boxed_str()))
            .unwrap_or(CellValue::Null),
        _ => row
            .try_get::<_, Option<String>>(index)
            .ok()
            .flatten()
            .map(|v| CellValue::Text(v.into_boxed_str()))
            .or_else(|| {
                row.try_get::<_, Option<FallbackText>>(index)
                    .ok()
                    .flatten()
                    .map(|v| CellValue::Text(v.0.into_boxed_str()))
            })
            .unwrap_or(CellValue::Null),
    }
}

/// Split SQL text into individual statements on `;` boundaries, respecting
/// string literals (`'...'`), dollar-quoted strings (`$$...$$` / `$tag$...$tag$`),
/// PostgreSQL-aware SQL statement splitter. Delegates to the shared
/// implementation in `sakidb_core::sql` with dollar-quoting enabled.
pub fn split_sql_statements(sql: &str) -> Vec<&str> {
    static PG_OPTS: SqlSplitOptions = SqlSplitOptions { dollar_quoting: true };
    split_sql_statements_with(sql, &PG_OPTS)
}

pub async fn execute_multi(
    pool: &Pool,
    sql: &str,
    conn_id: &ConnectionId,
    cancel_tokens: &Arc<DashMap<ConnectionId, CancelToken>>,
) -> Result<MultiQueryResult, SakiError> {
    let total_start = Instant::now();
    let statements = split_sql_statements(sql);

    if statements.is_empty() {
        return Ok(MultiQueryResult {
            results: vec![],
            total_execution_time_ms: 0,
        });
    }

    debug!(statements = statements.len(), "executing multi query");

    let mut results = Vec::with_capacity(statements.len());

    for stmt in &statements {
        let result = execute_query(pool, stmt, conn_id, cancel_tokens).await?;
        results.push(result);
    }

    let total_execution_time_ms = total_start.elapsed().as_millis() as u64;

    info!(statements = results.len(), elapsed_ms = total_execution_time_ms, "multi query complete");

    Ok(MultiQueryResult {
        results,
        total_execution_time_ms,
    })
}

pub async fn execute_export_cursor(
    pool: &Pool,
    sql: &str,
    batch_size: usize,
    on_batch: &mut (dyn FnMut(&[ColumnDef], &[CellValue], u64) -> std::result::Result<(), SakiError> + Send),
    cancel_flag: &AtomicBool,
) -> std::result::Result<u64, SakiError> {
    debug!(batch_size, "opening export cursor");
    let client = pool
        .get()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;

    // Begin transaction (cursors require one)
    client
        .batch_execute("BEGIN")
        .await
        .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;

    // Declare cursor
    let declare_sql = format!(
        "DECLARE _export_cursor NO SCROLL CURSOR FOR {sql}"
    );
    if let Err(e) = client.batch_execute(&declare_sql).await {
        // Rollback the BEGIN transaction on declare failure
        let _ = client.batch_execute("ROLLBACK").await;
        return Err(SakiError::QueryFailed(format_pg_error(&e)));
    }

    let fetch_sql = format!("FETCH {batch_size} FROM _export_cursor");
    let mut total_rows: u64 = 0;
    let mut columns: Vec<ColumnDef> = Vec::new();
    let mut cells: Vec<CellValue> = Vec::new();

    loop {
        // Check cancel flag before each fetch
        if cancel_flag.load(Ordering::Relaxed) {
            info!(rows = total_rows, "export cursor cancelled");
            let _ = client.batch_execute("CLOSE _export_cursor; ROLLBACK").await;
            return Err(SakiError::Cancelled);
        }

        let params: Vec<&(dyn ToSql + Sync)> = vec![];
        let stream = client
            .query_raw(&fetch_sql, params)
            .await
            .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;
        futures_util::pin_mut!(stream);

        cells.clear();
        let mut batch_row_count: u64 = 0;

        while let Some(row) = stream
            .try_next()
            .await
            .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?
        {
            if columns.is_empty() {
                columns = row
                    .columns()
                    .iter()
                    .map(|c| ColumnDef {
                        name: c.name().to_string(),
                        data_type: c.type_().name().to_string(),
                    })
                    .collect();
                cells.reserve(columns.len() * batch_size);
            }
            let cols = row.columns();
            for i in 0..cols.len() {
                cells.push(pg_value_to_cell(&row, i, cols[i].type_()));
            }
            batch_row_count += 1;
        }

        // Empty batch = cursor exhausted
        if batch_row_count == 0 {
            break;
        }

        total_rows += batch_row_count;
        on_batch(&columns, &cells, total_rows)?;
    }

    // Cleanup
    client
        .batch_execute("CLOSE _export_cursor; COMMIT")
        .await
        .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;

    info!(rows = total_rows, "export cursor complete");

    Ok(total_rows)
}

