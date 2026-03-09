use std::time::Instant;

use std::borrow::Cow;

use rusqlite::{types::ValueRef, Connection, Statement};
use tracing::{debug, info};

use sakidb_core::sql::split_sql_statements;
use sakidb_core::types::*;
use sakidb_core::SakiError;

/// Maximum rows returned by unbounded execute before truncation.
const MAX_EXECUTE_ROWS: u64 = 100_000;

/// Extract column definitions from a prepared statement.
fn extract_column_defs(stmt: &Statement<'_>) -> Vec<ColumnDef> {
    let columns = stmt.columns();
    (0..stmt.column_count())
        .map(|i| ColumnDef {
            name: stmt.column_name(i).unwrap_or("?").to_string(),
            data_type: columns
                .get(i)
                .and_then(|c| c.decl_type())
                .unwrap_or("TEXT")
                .to_string(),
        })
        .collect()
}

/// Map a SQLite ValueRef to a CellValue using the actual storage class per cell.
pub(crate) fn sqlite_value_to_cell(value: ValueRef<'_>) -> CellValue {
    match value {
        ValueRef::Null => CellValue::Null,
        ValueRef::Integer(i) => CellValue::Int(i),
        ValueRef::Real(f) => CellValue::Float(f),
        ValueRef::Text(bytes) => {
            let boxed: Box<str> = match String::from_utf8_lossy(bytes) {
                Cow::Borrowed(b) => b.into(),
                Cow::Owned(o) => o.into_boxed_str(),
            };
            CellValue::Text(boxed)
        }
        ValueRef::Blob(bytes) => CellValue::Bytes(bytes.to_vec().into_boxed_slice()),
    }
}

/// Classify a SQLite ValueRef into columnar type.
/// Returns: 0=Number, 1=Bool, 2=Text, 3=Bytes
fn classify_sqlite_value(value: ValueRef<'_>) -> u8 {
    match value {
        ValueRef::Integer(_) | ValueRef::Real(_) => 0,
        ValueRef::Blob(_) => 3,
        _ => 2, // Null, Text
    }
}

/// Push a SQLite value into the appropriate ColumnStorage.
fn push_columnar_value(value: ValueRef<'_>, col_type: u8, storage: &mut ColumnStorage) {
    match (col_type, storage) {
        (0, ColumnStorage::Number { nulls, values }) => match value {
            ValueRef::Null => {
                nulls.push(1);
                values.push(0.0);
            }
            ValueRef::Integer(i) => {
                nulls.push(0);
                values.push(i as f64);
            }
            ValueRef::Real(f) => {
                nulls.push(0);
                values.push(f);
            }
            _ => {
                nulls.push(1);
                values.push(0.0);
            }
        },
        (1, ColumnStorage::Bool { nulls, values }) => match value {
            ValueRef::Null => {
                nulls.push(1);
                values.push(0);
            }
            ValueRef::Integer(i) => {
                nulls.push(0);
                values.push(if i != 0 { 1 } else { 0 });
            }
            _ => {
                nulls.push(1);
                values.push(0);
            }
        },
        (2, ColumnStorage::Text { nulls, offsets, data }) => match value {
            ValueRef::Null => {
                nulls.push(1);
                offsets.push(data.len() as u32);
            }
            ValueRef::Text(bytes) => {
                nulls.push(0);
                data.extend_from_slice(bytes);
                offsets.push(data.len() as u32);
            }
            other => {
                nulls.push(0);
                let s = format!("{other:?}");
                data.extend_from_slice(s.as_bytes());
                offsets.push(data.len() as u32);
            }
        },
        (3, ColumnStorage::Bytes { nulls, offsets, data }) => match value {
            ValueRef::Null => {
                nulls.push(1);
                offsets.push(data.len() as u32);
            }
            ValueRef::Blob(bytes) => {
                nulls.push(0);
                data.extend_from_slice(bytes);
                offsets.push(data.len() as u32);
            }
            _ => {
                nulls.push(1);
                offsets.push(data.len() as u32);
            }
        },
        _ => unreachable!(),
    }
}

fn make_columnar_storage(col_type: u8, cap: usize) -> ColumnStorage {
    match col_type {
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
    }
}

pub fn execute_query(conn: &Connection, sql: &str) -> Result<QueryResult, SakiError> {
    let start = Instant::now();

    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

    let col_count = stmt.column_count();
    let columns = extract_column_defs(&stmt);

    let mut cells: Vec<CellValue> = Vec::with_capacity(col_count * 128);
    let mut row_count: u64 = 0;
    let mut truncated = false;

    let mut rows = stmt
        .query([])
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

    while let Some(row) = rows
        .next()
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?
    {
        for i in 0..col_count {
            let value = row
                .get_ref(i)
                .map_err(|e| SakiError::QueryFailed(e.to_string()))?;
            cells.push(sqlite_value_to_cell(value));
        }
        row_count += 1;

        if row_count >= MAX_EXECUTE_ROWS {
            truncated = true;
            break;
        }
    }

    let execution_time_ms = start.elapsed().as_millis() as u64;
    debug!(rows = row_count, cols = col_count, elapsed_ms = execution_time_ms, truncated, "query complete");

    Ok(QueryResult {
        columns,
        cells,
        row_count,
        execution_time_ms,
        truncated,
    })
}

pub fn execute_query_columnar(conn: &Connection, sql: &str) -> Result<ColumnarResult, SakiError> {
    let start = Instant::now();

    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

    let col_count = stmt.column_count();
    let columns = extract_column_defs(&stmt);

    let mut col_types: Vec<u8> = vec![2; col_count]; // default to Text
    let mut col_storages: Vec<ColumnStorage> = Vec::new();
    let mut row_count: u64 = 0;
    let mut truncated = false;
    let mut types_determined = false;

    let mut rows = stmt
        .query([])
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

    while let Some(row) = rows
        .next()
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?
    {
        // Determine column types from first row with non-null values
        if !types_determined {
            for (i, col_type) in col_types.iter_mut().enumerate() {
                let value = row
                    .get_ref(i)
                    .map_err(|e| SakiError::QueryFailed(e.to_string()))?;
                if !matches!(value, ValueRef::Null) {
                    *col_type = classify_sqlite_value(value);
                }
            }
            let cap = 1024usize;
            col_storages = col_types.iter().map(|&t| make_columnar_storage(t, cap)).collect();
            types_determined = true;
        }

        for i in 0..col_count {
            let value = row
                .get_ref(i)
                .map_err(|e| SakiError::QueryFailed(e.to_string()))?;
            push_columnar_value(value, col_types[i], &mut col_storages[i]);
        }
        row_count += 1;

        if row_count >= MAX_EXECUTE_ROWS {
            truncated = true;
            break;
        }
    }

    // Handle empty result set
    if !types_determined {
        col_storages = col_types.iter().map(|&t| make_columnar_storage(t, 0)).collect();
    }

    let execution_time_ms = start.elapsed().as_millis() as u64;
    debug!(rows = row_count, cols = col_count, elapsed_ms = execution_time_ms, truncated, "columnar query complete");

    Ok(ColumnarResult {
        columns,
        column_data: col_storages,
        row_count,
        execution_time_ms,
        truncated,
    })
}

pub fn execute_multi(conn: &Connection, sql: &str) -> Result<MultiQueryResult, SakiError> {
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
        results.push(execute_query(conn, stmt)?);
    }

    let total_execution_time_ms = total_start.elapsed().as_millis() as u64;
    info!(statements = results.len(), elapsed_ms = total_execution_time_ms, "multi query complete");

    Ok(MultiQueryResult {
        results,
        total_execution_time_ms,
    })
}

pub fn execute_multi_columnar(
    conn: &Connection,
    sql: &str,
) -> Result<MultiColumnarResult, SakiError> {
    let start = Instant::now();
    let statements = split_sql_statements(sql);

    debug!(statements = statements.len(), "executing multi columnar");

    let mut results = Vec::with_capacity(statements.len());
    for stmt in &statements {
        if stmt.is_empty() {
            continue;
        }
        results.push(execute_query_columnar(conn, stmt)?);
    }

    let elapsed = start.elapsed().as_millis() as u64;
    info!(statements = results.len(), elapsed_ms = elapsed, "multi columnar complete");

    Ok(MultiColumnarResult {
        results,
        total_execution_time_ms: elapsed,
    })
}

pub fn execute_paged(
    conn: &Connection,
    sql: &str,
    page: usize,
    page_size: usize,
) -> Result<PagedResult, SakiError> {
    let start = Instant::now();
    debug!(page, page_size, "executing paged query");

    let offset = page * page_size;
    let paged_sql = format!("SELECT * FROM ({sql}) LIMIT {page_size} OFFSET {offset}");

    // Only get count on first page
    let total_rows_estimate = if page == 0 {
        let count_sql = format!("SELECT COUNT(*) FROM ({sql})");
        conn.query_row(&count_sql, [], |row| row.get::<_, i64>(0))
            .ok()
    } else {
        None
    };

    let mut stmt = conn
        .prepare(&paged_sql)
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

    let col_count = stmt.column_count();
    let columns = extract_column_defs(&stmt);

    let mut cells: Vec<CellValue> = Vec::with_capacity(col_count * page_size);
    let mut row_count: u64 = 0;

    let mut rows = stmt
        .query([])
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

    while let Some(row) = rows
        .next()
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?
    {
        for i in 0..col_count {
            let value = row
                .get_ref(i)
                .map_err(|e| SakiError::QueryFailed(e.to_string()))?;
            cells.push(sqlite_value_to_cell(value));
        }
        row_count += 1;
    }

    let execution_time_ms = start.elapsed().as_millis() as u64;
    debug!(page, page_size, rows = row_count, total_estimate = ?total_rows_estimate, elapsed_ms = execution_time_ms, "paged query complete");

    Ok(PagedResult {
        columns,
        cells,
        row_count,
        page,
        page_size,
        total_rows_estimate,
        execution_time_ms,
    })
}

pub fn execute_batch(conn: &Connection, sql: &str) -> Result<(), SakiError> {
    let t0 = Instant::now();
    debug!("executing batch");
    conn.execute_batch(sql)
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;
    debug!(elapsed_ms = t0.elapsed().as_millis() as u64, "batch complete");
    Ok(())
}

#[allow(clippy::type_complexity)]
pub fn execute_export(
    conn: &Connection,
    sql: &str,
    batch_size: usize,
    on_batch: &mut (dyn FnMut(&[ColumnDef], &[CellValue], u64) -> Result<(), SakiError> + Send),
    cancel_flag: &std::sync::atomic::AtomicBool,
) -> Result<u64, SakiError> {
    use std::sync::atomic::Ordering;

    debug!(batch_size, "starting export");

    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

    let col_count = stmt.column_count();
    let columns = extract_column_defs(&stmt);

    let mut cells: Vec<CellValue> = Vec::with_capacity(col_count * batch_size);
    let mut total_rows: u64 = 0;
    let mut batch_row_count: u64 = 0;

    let mut rows = stmt
        .query([])
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

    while let Some(row) = rows
        .next()
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?
    {
        if cancel_flag.load(Ordering::Relaxed) {
            info!(rows = total_rows, "export cancelled");
            return Err(SakiError::Cancelled);
        }

        for i in 0..col_count {
            let value = row
                .get_ref(i)
                .map_err(|e| SakiError::QueryFailed(e.to_string()))?;
            cells.push(sqlite_value_to_cell(value));
        }
        batch_row_count += 1;
        total_rows += 1;

        if batch_row_count >= batch_size as u64 {
            on_batch(&columns, &cells, total_rows)?;
            cells.clear();
            batch_row_count = 0;
        }
    }

    // Flush remaining
    if batch_row_count > 0 {
        on_batch(&columns, &cells, total_rows)?;
    }

    info!(rows = total_rows, "export complete");
    Ok(total_rows)
}

