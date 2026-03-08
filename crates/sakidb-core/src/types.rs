use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Engine types ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineType {
    Postgres,
    Sqlite,
    Redis,
    MongoDB,
    DuckDB,
    ClickHouse,
}

impl std::fmt::Display for EngineType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Postgres => write!(f, "postgres"),
            Self::Sqlite => write!(f, "sqlite"),
            Self::Redis => write!(f, "redis"),
            Self::MongoDB => write!(f, "mongodb"),
            Self::DuckDB => write!(f, "duckdb"),
            Self::ClickHouse => write!(f, "clickhouse"),
        }
    }
}

impl std::str::FromStr for EngineType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "postgres" | "postgresql" => Ok(Self::Postgres),
            "sqlite" => Ok(Self::Sqlite),
            "redis" => Ok(Self::Redis),
            "mongodb" => Ok(Self::MongoDB),
            "duckdb" => Ok(Self::DuckDB),
            "clickhouse" => Ok(Self::ClickHouse),
            _ => Err(format!("unknown engine: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineCapabilities {
    // Trait-level capabilities
    pub sql: bool,
    pub introspection: bool,
    pub export: bool,
    pub restore: bool,
    pub key_value: bool,
    pub document: bool,

    // Feature-level granularity within introspection
    pub schemas: bool,
    pub tables: bool,
    pub views: bool,
    pub materialized_views: bool,
    pub functions: bool,
    pub sequences: bool,
    pub indexes: bool,
    pub triggers: bool,
    pub partitions: bool,
    pub foreign_tables: bool,
    pub explain: bool,
    pub multi_database: bool,
}

/// Returned by connect commands — bundles the runtime ID with engine capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectResult {
    pub runtime_id: String,
    pub capabilities: EngineCapabilities,
}

// ── Connection types ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConnectionId(pub Uuid);

impl ConnectionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub engine: EngineType,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub ssl_mode: SslMode,
    /// Engine-specific parameters (e.g. file_path for SQLite, db number for Redis)
    #[serde(default)]
    pub options: HashMap<String, String>,
}

impl std::fmt::Debug for ConnectionConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionConfig")
            .field("engine", &self.engine)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("database", &self.database)
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .field("ssl_mode", &self.ssl_mode)
            .field("options", &self.options)
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SslMode {
    Disable,
    Prefer,
    Require,
}

impl Default for SslMode {
    fn default() -> Self {
        Self::Prefer
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CellValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(Box<str>),
    Bytes(Box<[u8]>),
    Json(Box<str>),
    Timestamp(Box<str>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<ColumnDef>,
    pub cells: Vec<CellValue>,      // flat: cells[row * num_cols + col]
    pub row_count: u64,
    pub execution_time_ms: u64,
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MultiQueryResult {
    pub results: Vec<QueryResult>,
    pub total_execution_time_ms: u64,
}

// ── Columnar result types (low-memory wire format for query results) ──

/// Per-column storage. Avoids per-cell enum overhead.
#[derive(Debug)]
pub enum ColumnStorage {
    /// Int and float columns stored as f64 (matches JS number representation).
    Number { nulls: Vec<u8>, values: Vec<f64> },
    /// Boolean columns: 0 or 1 per row.
    Bool { nulls: Vec<u8>, values: Vec<u8> },
    /// Variable-length UTF-8 strings (text, json, timestamp, uuid, etc.).
    /// offsets has length row_count + 1. Row i's bytes are data[offsets[i]..offsets[i+1]].
    Text { nulls: Vec<u8>, offsets: Vec<u32>, data: Vec<u8> },
    /// Variable-length byte arrays (bytea).
    Bytes { nulls: Vec<u8>, offsets: Vec<u32>, data: Vec<u8> },
}

#[derive(Debug)]
pub struct ColumnarResult {
    pub columns: Vec<ColumnDef>,
    pub column_data: Vec<ColumnStorage>,
    pub row_count: u64,
    pub execution_time_ms: u64,
    pub truncated: bool,
}

#[derive(Debug)]
pub struct MultiColumnarResult {
    pub results: Vec<ColumnarResult>,
    pub total_execution_time_ms: u64,
}

impl ColumnarResult {
    /// Convenience wrapper — allocates a new buffer and encodes into it.
    /// Prefer `encode_into` when writing into an existing buffer.
    pub fn encode(self) -> Vec<u8> {
        let est = self.estimate_size();
        let mut buf = Vec::with_capacity(est);
        self.encode_into(&mut buf);
        buf
    }

    /// Encode to compact binary format for IPC transfer.
    /// Consumes `self` so each column's storage is freed after being written,
    /// reducing peak memory from 2× data size to ~1×.
    ///
    /// Format:
    /// - Header: u32 col_count, u64 row_count, u64 exec_time_ms, u8 truncated, 4 bytes padding
    /// - Column defs: (u16 name_len, name bytes, u16 type_len, type bytes) x col_count
    /// - Column data: (u8 type_tag, nulls, type-specific data) x col_count
    pub fn encode_into(self, buf: &mut Vec<u8>) {
        // NOTE: intentionally no bulk reserve() here — pre-allocating estimate_size()
        // would hold ~full output capacity while storage is still alive, doubling peak.
        // Instead, Vec grows naturally; each column is freed after writing.

        // Header (25 bytes)
        buf.extend_from_slice(&(self.columns.len() as u32).to_le_bytes());
        buf.extend_from_slice(&self.row_count.to_le_bytes());
        buf.extend_from_slice(&self.execution_time_ms.to_le_bytes());
        buf.push(if self.truncated { 1 } else { 0 });
        buf.extend_from_slice(&[0u8; 4]); // padding for alignment

        // Column definitions
        for col in &self.columns {
            let name_bytes = col.name.as_bytes();
            buf.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            buf.extend_from_slice(name_bytes);
            let type_bytes = col.data_type.as_bytes();
            buf.extend_from_slice(&(type_bytes.len() as u16).to_le_bytes());
            buf.extend_from_slice(type_bytes);
        }

        // Column data — consuming iterator frees each column's storage after writing
        for col_data in self.column_data {
            match col_data {
                ColumnStorage::Number { nulls, values } => {
                    buf.push(0); // type tag
                    buf.extend_from_slice(&nulls);
                    // nulls dropped here
                    // Pad to 8-byte alignment for Float64Array zero-copy on JS side
                    let padding = (8 - (buf.len() % 8)) % 8;
                    buf.extend_from_slice(&[0u8; 7][..padding]);
                    // Bulk write: cast Vec<f64> directly to bytes (all targets are little-endian)
                    let byte_slice = unsafe {
                        std::slice::from_raw_parts(values.as_ptr() as *const u8, values.len() * 8)
                    };
                    buf.extend_from_slice(byte_slice);
                    // values dropped here
                }
                ColumnStorage::Bool { nulls, values } => {
                    buf.push(1); // type tag
                    buf.extend_from_slice(&nulls);
                    buf.extend_from_slice(&values);
                    // nulls + values dropped here
                }
                ColumnStorage::Text { nulls, offsets, data } => {
                    buf.push(2); // type tag
                    buf.extend_from_slice(&nulls);
                    drop(nulls);
                    buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
                    buf.extend_from_slice(&data);
                    drop(data); // free text data immediately (can be 100+ MB)
                    for offset in offsets {
                        buf.extend_from_slice(&offset.to_le_bytes());
                    }
                }
                ColumnStorage::Bytes { nulls, offsets, data } => {
                    buf.push(3); // type tag
                    buf.extend_from_slice(&nulls);
                    drop(nulls);
                    buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
                    buf.extend_from_slice(&data);
                    drop(data); // free byte data immediately
                    for offset in offsets {
                        buf.extend_from_slice(&offset.to_le_bytes());
                    }
                }
            }
        }
    }

    fn estimate_size(&self) -> usize {
        let mut size = 25; // header
        for col in &self.columns {
            size += 4 + col.name.len() + col.data_type.len();
        }
        let rc = self.row_count as usize;
        for col_data in &self.column_data {
            size += 1 + rc; // type tag + nulls
            match col_data {
                ColumnStorage::Number { values, .. } => size += 8 + values.len() * 8,
                ColumnStorage::Bool { values, .. } => size += values.len(),
                ColumnStorage::Text { offsets, data, .. }
                | ColumnStorage::Bytes { offsets, data, .. } => {
                    size += 4 + data.len() + offsets.len() * 4;
                }
            }
        }
        size
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PagedResult {
    pub columns: Vec<ColumnDef>,
    pub cells: Vec<CellValue>,      // flat: cells[row * num_cols + col]
    pub row_count: u64,
    pub page: usize,
    pub page_size: usize,
    pub total_rows_estimate: Option<i64>,
    pub execution_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseInfo {
    pub name: String,
    pub is_template: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaInfo {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub name: String,
    pub row_count_estimate: Option<i64>,
    pub size_bytes: Option<i64>,
    pub is_partition: bool,
    pub parent_table: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub is_nullable: bool,
    pub is_primary_key: bool,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewInfo {
    pub name: String,
    pub is_updatable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterializedViewInfo {
    pub name: String,
    pub row_count_estimate: Option<i64>,
    pub is_populated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionInfo {
    pub name: String,
    pub kind: String,
    pub return_type: String,
    pub argument_types: String,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceInfo {
    pub name: String,
    pub data_type: String,
    pub last_value: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    pub name: String,
    pub table_name: String,
    pub columns: String,
    pub is_unique: bool,
    pub is_primary: bool,
    pub index_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeignTableInfo {
    pub name: String,
    pub server_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerInfo {
    pub name: String,
    pub table_name: String,
    pub event: String,
    pub timing: String,
    pub for_each: String,
    pub function_name: String,
    pub function_schema: String,
    pub condition: Option<String>,
    pub is_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeignKeyInfo {
    pub constraint_name: String,
    pub columns: Vec<String>,
    pub foreign_table_schema: String,
    pub foreign_table_name: String,
    pub foreign_columns: Vec<String>,
    pub on_update: String,
    pub on_delete: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckConstraintInfo {
    pub constraint_name: String,
    pub check_clause: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniqueConstraintInfo {
    pub constraint_name: String,
    pub columns: Vec<String>,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionInfo {
    pub strategy: String,
    pub partition_key: String,
    pub partitions: Vec<PartitionDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionDetail {
    pub name: String,
    pub expression: String,
    pub row_count_estimate: Option<i64>,
}

// ── Completion types (editor autocomplete) ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionTable {
    pub name: String,
    pub kind: String, // "table", "view", "materialized_view"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionBundle {
    pub tables: Vec<CompletionTable>,
    pub functions: Vec<FunctionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionColumn {
    pub name: String,
    pub data_type: String,
    pub is_primary_key: bool,
    pub is_nullable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErdData {
    pub tables: Vec<TableInfo>,
    pub columns: HashMap<String, Vec<ColumnInfo>>,
    /// Foreign keys grouped by source table name
    pub foreign_keys: HashMap<String, Vec<ForeignKeyInfo>>,
}

// ── Restore types ──

pub struct RestoreOptions {
    pub schema: Option<String>,
    pub continue_on_error: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RestoreProgress {
    pub bytes_read: u64,
    pub total_bytes: u64,
    pub statements_executed: u64,
    pub errors_skipped: u64,
    pub phase: String,
    pub elapsed_ms: u64,
    pub error: Option<String>,
    pub error_messages: Vec<String>,
}

// ── Export types ──

/// Callback type for streaming export batches.
/// Parameters: (columns, cells as flat array, total_rows_so_far)
pub type ExportBatchFn =
    dyn Fn(&[ColumnDef], &[CellValue], u64) -> crate::error::Result<()> + Send + Sync;

