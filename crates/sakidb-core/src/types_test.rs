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
