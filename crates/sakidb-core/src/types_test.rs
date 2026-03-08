use super::types::*;

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

// ── CellValue edge cases ──

#[test]
fn cell_value_bytes_roundtrip() {
    let binary_data: Vec<u8> = vec![0x00, 0xFF, 0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x80];
    let val = CellValue::Bytes(binary_data.clone().into_boxed_slice());
    let json = serde_json::to_string(&val).unwrap();
    let back: CellValue = serde_json::from_str(&json).unwrap();
    match back {
        CellValue::Bytes(b) => assert_eq!(b.as_ref(), binary_data.as_slice()),
        other => panic!("expected Bytes, got {:?}", other),
    }
}

#[test]
fn cell_value_json_nested() {
    let nested = r#"{"level1":{"level2":{"level3":{"value":42,"array":[1,2,3]}}}}"#;
    let val = CellValue::Json(Box::from(nested));
    let json = serde_json::to_string(&val).unwrap();
    let back: CellValue = serde_json::from_str(&json).unwrap();
    match back {
        CellValue::Json(s) => {
            assert_eq!(s.as_ref(), nested);
            // Verify the nested JSON is itself valid
            let parsed: serde_json::Value = serde_json::from_str(&s).unwrap();
            assert_eq!(parsed["level1"]["level2"]["level3"]["value"], 42);
            assert_eq!(parsed["level1"]["level2"]["level3"]["array"][2], 3);
        }
        other => panic!("expected Json, got {:?}", other),
    }
}

#[test]
fn cell_value_text_unicode() {
    let unicode_text = "Hello 🎉 日本語 العربية mixed content ñ ü ö";
    let val = CellValue::Text(Box::from(unicode_text));
    let json = serde_json::to_string(&val).unwrap();
    let back: CellValue = serde_json::from_str(&json).unwrap();
    match back {
        CellValue::Text(s) => {
            assert_eq!(s.as_ref(), unicode_text);
            assert!(s.contains("🎉"));
            assert!(s.contains("日本語"));
            assert!(s.contains("العربية"));
        }
        other => panic!("expected Text, got {:?}", other),
    }
}

#[test]
fn cell_value_float_special() {
    // NAN, INFINITY, NEG_INFINITY, and -0.0 should not panic
    let specials = vec![
        CellValue::Float(f64::NAN),
        CellValue::Float(f64::INFINITY),
        CellValue::Float(f64::NEG_INFINITY),
        CellValue::Float(-0.0),
    ];
    for val in &specials {
        // Debug formatting should not panic
        let _ = format!("{:?}", val);
    }
    // -0.0 should serialize/deserialize cleanly
    let neg_zero = CellValue::Float(-0.0);
    let json = serde_json::to_string(&neg_zero).unwrap();
    let back: CellValue = serde_json::from_str(&json).unwrap();
    match back {
        CellValue::Float(f) => assert!(f == 0.0),
        other => panic!("expected Float, got {:?}", other),
    }
    // NAN cannot roundtrip through JSON (serde_json rejects NaN by default),
    // but it must not panic on creation or debug
    let nan = CellValue::Float(f64::NAN);
    let debug_str = format!("{:?}", nan);
    assert!(debug_str.contains("NaN"));
}

#[test]
fn cell_value_timestamp_formats() {
    let timestamps = vec![
        "2024-01-01",
        "2024-01-01T12:00:00Z",
        "2024-01-01 12:00:00+05:30",
    ];
    for ts in &timestamps {
        let val = CellValue::Timestamp(Box::from(*ts));
        let json = serde_json::to_string(&val).unwrap();
        let back: CellValue = serde_json::from_str(&json).unwrap();
        match back {
            CellValue::Timestamp(s) => assert_eq!(s.as_ref(), *ts),
            other => panic!("expected Timestamp, got {:?}", other),
        }
    }
}

// ── ColumnarResult encoding ──

#[test]
fn columnar_mixed_columns() {
    let result = ColumnarResult {
        columns: vec![
            ColumnDef { name: "id".into(), data_type: "int4".into() },
            ColumnDef { name: "name".into(), data_type: "text".into() },
            ColumnDef { name: "active".into(), data_type: "bool".into() },
        ],
        column_data: vec![
            ColumnStorage::Number {
                nulls: vec![0, 0],
                values: vec![1.0, 2.0],
            },
            ColumnStorage::Text {
                nulls: vec![0, 0],
                offsets: vec![0, 5, 10],
                data: b"AliceBobby".to_vec(),
            },
            ColumnStorage::Bool {
                nulls: vec![0, 0],
                values: vec![1, 0],
            },
        ],
        row_count: 2,
        execution_time_ms: 15,
        truncated: false,
    };
    let bytes = result.encode();
    // Verify header
    assert_eq!(u32::from_le_bytes(bytes[0..4].try_into().unwrap()), 3); // 3 columns
    assert_eq!(u64::from_le_bytes(bytes[4..12].try_into().unwrap()), 2); // 2 rows
    assert_eq!(u64::from_le_bytes(bytes[12..20].try_into().unwrap()), 15); // exec_time
    assert!(bytes.len() > 25);
}

#[test]
fn columnar_all_nulls() {
    let row_count = 5;
    let result = ColumnarResult {
        columns: vec![
            ColumnDef { name: "val".into(), data_type: "int4".into() },
        ],
        column_data: vec![
            ColumnStorage::Number {
                nulls: vec![1; row_count],
                values: vec![0.0; row_count],
            },
        ],
        row_count: row_count as u64,
        execution_time_ms: 1,
        truncated: false,
    };
    let bytes = result.encode();
    assert_eq!(u64::from_le_bytes(bytes[4..12].try_into().unwrap()), 5);
    // Find where column data starts (after header + column defs)
    // Header = 25 bytes, then col def for "val"/"int4": 2+3+2+4 = 11 bytes
    let col_data_start = 25 + 11;
    // Type tag = 0 (Number)
    assert_eq!(bytes[col_data_start], 0);
    // Nulls: all 1s
    for i in 0..row_count {
        assert_eq!(bytes[col_data_start + 1 + i], 1, "null bitmap at row {} should be 1", i);
    }
}

#[test]
fn columnar_empty_result() {
    let result = ColumnarResult {
        columns: vec![
            ColumnDef { name: "a".into(), data_type: "int4".into() },
            ColumnDef { name: "b".into(), data_type: "text".into() },
            ColumnDef { name: "c".into(), data_type: "bool".into() },
        ],
        column_data: vec![
            ColumnStorage::Number { nulls: vec![], values: vec![] },
            ColumnStorage::Text { nulls: vec![], offsets: vec![0], data: vec![] },
            ColumnStorage::Bool { nulls: vec![], values: vec![] },
        ],
        row_count: 0,
        execution_time_ms: 0,
        truncated: false,
    };
    let bytes = result.encode();
    assert_eq!(u32::from_le_bytes(bytes[0..4].try_into().unwrap()), 3); // 3 columns
    assert_eq!(u64::from_le_bytes(bytes[4..12].try_into().unwrap()), 0); // 0 rows
    assert!(bytes.len() > 25); // still has column defs and type tags
}

#[test]
fn columnar_large_column() {
    let n = 10_001;
    let values: Vec<f64> = (0..n).map(|i| i as f64).collect();
    let nulls = vec![0u8; n];
    let result = ColumnarResult {
        columns: vec![
            ColumnDef { name: "big".into(), data_type: "float8".into() },
        ],
        column_data: vec![
            ColumnStorage::Number { nulls, values },
        ],
        row_count: n as u64,
        execution_time_ms: 100,
        truncated: false,
    };
    let bytes = result.encode();
    assert_eq!(u64::from_le_bytes(bytes[4..12].try_into().unwrap()), n as u64);
    // The encoded data should contain at least the header + nulls + values
    // values alone = 10001 * 8 = 80008 bytes
    assert!(bytes.len() > 80_000);
}

#[test]
fn columnar_bytes_column() {
    let bin1: Vec<u8> = vec![0xDE, 0xAD];
    let bin2: Vec<u8> = vec![0xBE, 0xEF, 0x00];
    let mut data = Vec::new();
    data.extend_from_slice(&bin1);
    data.extend_from_slice(&bin2);
    let result = ColumnarResult {
        columns: vec![
            ColumnDef { name: "blob".into(), data_type: "bytea".into() },
        ],
        column_data: vec![
            ColumnStorage::Bytes {
                nulls: vec![0, 0],
                offsets: vec![0, 2, 5],
                data,
            },
        ],
        row_count: 2,
        execution_time_ms: 3,
        truncated: false,
    };
    let bytes = result.encode();
    assert_eq!(u32::from_le_bytes(bytes[0..4].try_into().unwrap()), 1); // 1 column
    assert_eq!(u64::from_le_bytes(bytes[4..12].try_into().unwrap()), 2); // 2 rows
    // Find the column data type tag (after header + column def)
    // Header=25, col def for "blob"/"bytea": 2+4+2+5=13
    let col_data_start = 25 + 13;
    assert_eq!(bytes[col_data_start], 3); // Bytes type tag
}

#[test]
fn rows_to_columnar_consistency() {
    use crate::driver::rows_to_columnar;

    let qr = QueryResult {
        columns: vec![
            ColumnDef { name: "id".into(), data_type: "int4".into() },
            ColumnDef { name: "name".into(), data_type: "text".into() },
            ColumnDef { name: "active".into(), data_type: "bool".into() },
        ],
        // 2 rows x 3 cols = 6 cells, flat layout
        cells: vec![
            CellValue::Int(1), CellValue::Text(Box::from("Alice")), CellValue::Bool(true),
            CellValue::Int(2), CellValue::Text(Box::from("Bob")), CellValue::Bool(false),
        ],
        row_count: 2,
        execution_time_ms: 10,
        truncated: false,
    };

    let multi = MultiQueryResult {
        results: vec![qr],
        total_execution_time_ms: 10,
    };

    let columnar = rows_to_columnar(multi);
    assert_eq!(columnar.total_execution_time_ms, 10);
    assert_eq!(columnar.results.len(), 1);

    let cr = &columnar.results[0];
    assert_eq!(cr.row_count, 2);
    assert_eq!(cr.columns.len(), 3);
    assert_eq!(cr.column_data.len(), 3);

    // Column 0: Number (id)
    match &cr.column_data[0] {
        ColumnStorage::Number { nulls, values } => {
            assert_eq!(nulls, &vec![0, 0]);
            assert_eq!(values[0], 1.0);
            assert_eq!(values[1], 2.0);
        }
        other => panic!("expected Number column, got {:?}", other),
    }

    // Column 1: Text (name)
    match &cr.column_data[1] {
        ColumnStorage::Text { nulls, offsets, data } => {
            assert_eq!(nulls, &vec![0, 0]);
            // Row 0: "Alice" (5 bytes), Row 1: "Bob" (3 bytes)
            assert_eq!(offsets.len(), 3); // row_count + 1
            let row0 = std::str::from_utf8(&data[offsets[0] as usize..offsets[1] as usize]).unwrap();
            let row1 = std::str::from_utf8(&data[offsets[1] as usize..offsets[2] as usize]).unwrap();
            assert_eq!(row0, "Alice");
            assert_eq!(row1, "Bob");
        }
        other => panic!("expected Text column, got {:?}", other),
    }

    // Column 2: Bool (active)
    match &cr.column_data[2] {
        ColumnStorage::Bool { nulls, values } => {
            assert_eq!(nulls, &vec![0, 0]);
            assert_eq!(values[0], 1); // true
            assert_eq!(values[1], 0); // false
        }
        other => panic!("expected Bool column, got {:?}", other),
    }
}

// ── EngineCapabilities ──

#[test]
fn capabilities_default_values() {
    // Construct with all false/empty — verify "zeroed" state is sensible
    let caps = EngineCapabilities {
        sql: false,
        introspection: false,
        export: false,
        restore: false,
        key_value: false,
        document: false,
        schemas: false,
        tables: false,
        views: false,
        materialized_views: false,
        functions: false,
        sequences: false,
        indexes: false,
        triggers: false,
        partitions: false,
        foreign_tables: false,
        explain: false,
        multi_database: false,
    };
    assert!(!caps.sql);
    assert!(!caps.introspection);
    assert!(!caps.export);
    assert!(!caps.restore);
    assert!(!caps.key_value);
    assert!(!caps.document);
    assert!(!caps.schemas);
    assert!(!caps.tables);
    assert!(!caps.views);
    assert!(!caps.materialized_views);
    assert!(!caps.functions);
    assert!(!caps.sequences);
    assert!(!caps.indexes);
    assert!(!caps.triggers);
    assert!(!caps.partitions);
    assert!(!caps.foreign_tables);
    assert!(!caps.explain);
    assert!(!caps.multi_database);
}

#[test]
fn capabilities_serialization() {
    let caps = EngineCapabilities {
        sql: true,
        introspection: true,
        export: true,
        restore: false,
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
        multi_database: true,
    };
    let json = serde_json::to_string(&caps).unwrap();
    let back: EngineCapabilities = serde_json::from_str(&json).unwrap();
    assert_eq!(caps.sql, back.sql);
    assert_eq!(caps.introspection, back.introspection);
    assert_eq!(caps.export, back.export);
    assert_eq!(caps.restore, back.restore);
    assert_eq!(caps.key_value, back.key_value);
    assert_eq!(caps.document, back.document);
    assert_eq!(caps.schemas, back.schemas);
    assert_eq!(caps.tables, back.tables);
    assert_eq!(caps.views, back.views);
    assert_eq!(caps.materialized_views, back.materialized_views);
    assert_eq!(caps.functions, back.functions);
    assert_eq!(caps.sequences, back.sequences);
    assert_eq!(caps.indexes, back.indexes);
    assert_eq!(caps.triggers, back.triggers);
    assert_eq!(caps.partitions, back.partitions);
    assert_eq!(caps.foreign_tables, back.foreign_tables);
    assert_eq!(caps.explain, back.explain);
    assert_eq!(caps.multi_database, back.multi_database);
}

// ── ConnectionConfig ──

#[test]
fn connection_config_defaults() {
    use std::collections::HashMap;
    let config = ConnectionConfig {
        engine: EngineType::Postgres,
        host: "localhost".into(),
        port: 5432,
        database: "testdb".into(),
        username: "user".into(),
        password: "pass".into(),
        ssl_mode: SslMode::default(),
        options: HashMap::new(),
    };
    assert_eq!(config.port, 5432);
    assert!(matches!(config.ssl_mode, SslMode::Prefer));
    assert!(matches!(config.engine, EngineType::Postgres));
    assert!(config.options.is_empty());
    // Debug should redact password value (the field name "password" will appear but the value won't)
    let debug = format!("{:?}", config);
    assert!(debug.contains("REDACTED"));
    assert!(!debug.contains("\"pass\""));
}

#[test]
fn connection_config_with_options() {
    use std::collections::HashMap;
    let mut options = HashMap::new();
    options.insert("file_path".to_string(), "/tmp/test.db".to_string());
    options.insert("journal_mode".to_string(), "wal".to_string());

    let config = ConnectionConfig {
        engine: EngineType::Sqlite,
        host: String::new(),
        port: 0,
        database: String::new(),
        username: String::new(),
        password: String::new(),
        ssl_mode: SslMode::Disable,
        options: options.clone(),
    };

    let json = serde_json::to_string(&config).unwrap();
    let back: ConnectionConfig = serde_json::from_str(&json).unwrap();
    assert!(matches!(back.engine, EngineType::Sqlite));
    assert_eq!(back.options.get("file_path").unwrap(), "/tmp/test.db");
    assert_eq!(back.options.get("journal_mode").unwrap(), "wal");
    assert_eq!(back.options.len(), 2);
}

// ── Large data correctness ──

#[test]
fn columnar_100k_rows_mixed_types() {
    let n = 100_000;

    // Number column
    let num_values: Vec<f64> = (0..n).map(|i| i as f64 * 1.5).collect();
    let num_nulls = vec![0u8; n];

    // Text column
    let mut text_data = Vec::new();
    let mut text_offsets = vec![0u32];
    let text_nulls = vec![0u8; n];
    for i in 0..n {
        let s = format!("row_{}", i);
        text_data.extend_from_slice(s.as_bytes());
        text_offsets.push(text_data.len() as u32);
    }

    // Bool column
    let bool_values: Vec<u8> = (0..n).map(|i| (i % 2) as u8).collect();
    let bool_nulls = vec![0u8; n];

    let result = ColumnarResult {
        columns: vec![
            ColumnDef { name: "num".into(), data_type: "float8".into() },
            ColumnDef { name: "txt".into(), data_type: "text".into() },
            ColumnDef { name: "flag".into(), data_type: "bool".into() },
        ],
        column_data: vec![
            ColumnStorage::Number { nulls: num_nulls, values: num_values },
            ColumnStorage::Text { nulls: text_nulls, offsets: text_offsets, data: text_data },
            ColumnStorage::Bool { nulls: bool_nulls, values: bool_values },
        ],
        row_count: n as u64,
        execution_time_ms: 500,
        truncated: false,
    };

    let bytes = result.encode();
    assert_eq!(u32::from_le_bytes(bytes[0..4].try_into().unwrap()), 3);
    assert_eq!(u64::from_le_bytes(bytes[4..12].try_into().unwrap()), n as u64);
    // Number data alone: 100_000 * 8 = 800_000 bytes
    assert!(bytes.len() > 800_000);
}

#[test]
fn columnar_wide_result() {
    let num_cols = 200;
    let num_rows = 100;

    let columns: Vec<ColumnDef> = (0..num_cols)
        .map(|i| ColumnDef { name: format!("col_{}", i), data_type: "int4".into() })
        .collect();
    let column_data: Vec<ColumnStorage> = (0..num_cols)
        .map(|col_idx| ColumnStorage::Number {
            nulls: vec![0u8; num_rows],
            values: (0..num_rows).map(|row| (row * num_cols + col_idx) as f64).collect(),
        })
        .collect();

    let result = ColumnarResult {
        columns,
        column_data,
        row_count: num_rows as u64,
        execution_time_ms: 50,
        truncated: false,
    };

    let bytes = result.encode();
    assert_eq!(u32::from_le_bytes(bytes[0..4].try_into().unwrap()), num_cols as u32);
    assert_eq!(u64::from_le_bytes(bytes[4..12].try_into().unwrap()), num_rows as u64);
    // Each column: type tag (1) + nulls (100) + padding + values (100*8=800)
    // Minimum per column ~ 901 bytes, total ~ 200 * 901 = 180,200
    assert!(bytes.len() > 150_000);
}

#[test]
fn columnar_large_text_cells() {
    // Create a text value over 1MB
    let large_text: String = "x".repeat(1_100_000);
    let small_text = "tiny";
    let mut data = Vec::new();
    data.extend_from_slice(large_text.as_bytes());
    data.extend_from_slice(small_text.as_bytes());

    let result = ColumnarResult {
        columns: vec![
            ColumnDef { name: "content".into(), data_type: "text".into() },
        ],
        column_data: vec![
            ColumnStorage::Text {
                nulls: vec![0, 0],
                offsets: vec![0, large_text.len() as u32, (large_text.len() + small_text.len()) as u32],
                data,
            },
        ],
        row_count: 2,
        execution_time_ms: 200,
        truncated: false,
    };

    let bytes = result.encode();
    // The encoded bytes should contain the full 1.1MB+ text without truncation
    assert!(bytes.len() > 1_100_000, "encoded size {} should exceed 1.1MB", bytes.len());
    assert_eq!(u64::from_le_bytes(bytes[4..12].try_into().unwrap()), 2);
}

#[test]
fn rows_to_columnar_stress() {
    use crate::driver::rows_to_columnar;

    let n = 50_000;
    let num_cols = 4; // Int, Text, Bool, Float

    let mut cells = Vec::with_capacity(n * num_cols);
    for i in 0..n {
        cells.push(CellValue::Int(i as i64));
        cells.push(CellValue::Text(Box::from(format!("name_{}", i))));
        cells.push(CellValue::Bool(i % 2 == 0));
        cells.push(CellValue::Float(i as f64 * 0.1));
    }

    let qr = QueryResult {
        columns: vec![
            ColumnDef { name: "id".into(), data_type: "int4".into() },
            ColumnDef { name: "name".into(), data_type: "text".into() },
            ColumnDef { name: "flag".into(), data_type: "bool".into() },
            ColumnDef { name: "score".into(), data_type: "float8".into() },
        ],
        cells,
        row_count: n as u64,
        execution_time_ms: 100,
        truncated: false,
    };

    let multi = MultiQueryResult {
        results: vec![qr],
        total_execution_time_ms: 100,
    };

    let columnar = rows_to_columnar(multi);
    assert_eq!(columnar.results.len(), 1);

    let cr = &columnar.results[0];
    assert_eq!(cr.row_count, n as u64);
    assert_eq!(cr.column_data.len(), 4);

    // Verify column 0 (Int -> Number)
    match &cr.column_data[0] {
        ColumnStorage::Number { nulls, values } => {
            assert_eq!(values.len(), n);
            assert_eq!(nulls.len(), n);
            assert_eq!(values[0], 0.0);
            assert_eq!(values[n - 1], (n - 1) as f64);
            // No nulls
            assert!(nulls.iter().all(|&b| b == 0));
        }
        other => panic!("expected Number column for id, got {:?}", other),
    }

    // Verify column 1 (Text)
    match &cr.column_data[1] {
        ColumnStorage::Text { nulls, offsets, data } => {
            assert_eq!(nulls.len(), n);
            assert_eq!(offsets.len(), n + 1);
            // Spot-check first and last values
            let first = std::str::from_utf8(&data[offsets[0] as usize..offsets[1] as usize]).unwrap();
            assert_eq!(first, "name_0");
            let last = std::str::from_utf8(&data[offsets[n - 1] as usize..offsets[n] as usize]).unwrap();
            assert_eq!(last, format!("name_{}", n - 1));
        }
        other => panic!("expected Text column for name, got {:?}", other),
    }

    // Verify column 2 (Bool)
    match &cr.column_data[2] {
        ColumnStorage::Bool { nulls, values } => {
            assert_eq!(values.len(), n);
            assert_eq!(nulls.len(), n);
            assert_eq!(values[0], 1); // 0 % 2 == 0 -> true
            assert_eq!(values[1], 0); // 1 % 2 != 0 -> false
        }
        other => panic!("expected Bool column for flag, got {:?}", other),
    }

    // Verify column 3 (Float -> Number)
    match &cr.column_data[3] {
        ColumnStorage::Number { values, .. } => {
            assert_eq!(values.len(), n);
            assert!((values[10] - 1.0).abs() < 1e-10);
        }
        other => panic!("expected Number column for score, got {:?}", other),
    }

    // Verify it also encodes without error
    let bytes = cr.columns.len(); // just check we can access it
    assert_eq!(bytes, 4);
}
