use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConnectionId(pub Uuid);

impl ConnectionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub ssl_mode: SslMode,
}

impl std::fmt::Debug for ConnectionConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionConfig")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("database", &self.database)
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .field("ssl_mode", &self.ssl_mode)
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
                    buf.extend_from_slice(&vec![0u8; padding]);
                    for v in values {
                        buf.extend_from_slice(&v.to_le_bytes());
                    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_id_uniqueness() {
        let a = ConnectionId::new();
        let b = ConnectionId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn cell_value_serialization_roundtrip() {
        let values = vec![
            CellValue::Null,
            CellValue::Bool(true),
            CellValue::Int(42),
            CellValue::Float(3.14),
            CellValue::Text("hello".into()),
            CellValue::Timestamp(Box::from("2024-01-01T00:00:00Z")),
        ];
        for val in &values {
            let json = serde_json::to_string(val).unwrap();
            let back: CellValue = serde_json::from_str(&json).unwrap();
            let _ = format!("{:?}", back);
        }
    }

    #[test]
    fn query_result_serialization() {
        let result = QueryResult {
            columns: vec![ColumnDef { name: "id".into(), data_type: "int4".into() }],
            cells: vec![CellValue::Int(1)],
            row_count: 1,
            execution_time_ms: 5,
            truncated: false,
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: QueryResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.columns.len(), 1);
        assert_eq!(back.cells.len(), 1);
        assert_eq!(back.row_count, 1);
    }

    #[test]
    fn ssl_mode_default() {
        let mode = SslMode::default();
        assert!(matches!(mode, SslMode::Prefer));
    }

    #[test]
    fn columnar_result_encode_roundtrip_numbers() {
        let result = ColumnarResult {
            columns: vec![
                ColumnDef { name: "id".into(), data_type: "int4".into() },
            ],
            column_data: vec![
                ColumnStorage::Number {
                    nulls: vec![0, 0, 1],
                    values: vec![1.0, 2.0, 0.0],
                },
            ],
            row_count: 3,
            execution_time_ms: 42,
            truncated: false,
        };
        let bytes = result.encode();
        // Verify header
        assert_eq!(u32::from_le_bytes(bytes[0..4].try_into().unwrap()), 1); // col_count
        assert_eq!(u64::from_le_bytes(bytes[4..12].try_into().unwrap()), 3); // row_count
        assert_eq!(u64::from_le_bytes(bytes[12..20].try_into().unwrap()), 42); // exec_time
        assert_eq!(bytes[20], 0); // not truncated
        assert!(bytes.len() > 25); // has column data
    }

    #[test]
    fn columnar_result_encode_text_column() {
        let result = ColumnarResult {
            columns: vec![
                ColumnDef { name: "name".into(), data_type: "text".into() },
            ],
            column_data: vec![
                ColumnStorage::Text {
                    nulls: vec![0, 1, 0],
                    offsets: vec![0, 5, 5, 10],
                    data: b"helloworld".to_vec(),
                },
            ],
            row_count: 3,
            execution_time_ms: 10,
            truncated: false,
        };
        let bytes = result.encode();
        assert!(bytes.len() > 25);
    }
}
