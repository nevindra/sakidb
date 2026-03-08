#![cfg(feature = "stress")]

use sakidb_core::types::*;

#[test]
fn stress_columnar_1m_rows() {
    let n = 1_000_000;
    let result = ColumnarResult {
        columns: vec![ColumnDef {
            name: "val".into(),
            data_type: "float8".into(),
        }],
        column_data: vec![ColumnStorage::Number {
            nulls: vec![0u8; n],
            values: (0..n).map(|i| i as f64).collect(),
        }],
        row_count: n as u64,
        execution_time_ms: 0,
        truncated: false,
    };

    let bytes = result.encode();

    // At minimum: 25 (header) + column defs + 1 (type tag) + 1M nulls + padding + 1M*8 f64 values
    assert!(
        bytes.len() > n * 8,
        "encoded size {} is too small for {n} f64 values",
        bytes.len()
    );

    // Sanity: should be roughly n*9 (nulls + f64s) + overhead
    let expected_min = n * 9; // 1 byte null + 8 bytes f64 per row
    assert!(
        bytes.len() >= expected_min,
        "encoded size {} is below expected minimum {}",
        bytes.len(),
        expected_min
    );
}

#[test]
fn stress_columnar_500_columns() {
    let num_cols = 500;
    let num_rows = 10_000;

    let columns: Vec<ColumnDef> = (0..num_cols)
        .map(|i| ColumnDef {
            name: format!("col_{i}"),
            data_type: "float8".into(),
        })
        .collect();

    let column_data: Vec<ColumnStorage> = (0..num_cols)
        .map(|col_idx| ColumnStorage::Number {
            nulls: vec![0u8; num_rows],
            values: (0..num_rows)
                .map(|row| (col_idx * num_rows + row) as f64)
                .collect(),
        })
        .collect();

    let result = ColumnarResult {
        columns,
        column_data,
        row_count: num_rows as u64,
        execution_time_ms: 0,
        truncated: false,
    };

    let bytes = result.encode();

    // Each column: 1 type tag + num_rows nulls + padding + num_rows*8 f64 values
    // Total data minimum: 500 * (10000 + 10000*8) = 500 * 90000 = 45MB
    let expected_min = num_cols * num_rows * 9;
    assert!(
        bytes.len() >= expected_min,
        "encoded size {} is below expected minimum {} for {num_cols} columns x {num_rows} rows",
        bytes.len(),
        expected_min
    );
}

#[test]
fn stress_cellvalue_10mb_text() {
    let size = 10 * 1024 * 1024; // 10 MB
    let large_text: String = "A".repeat(size);
    let cell = CellValue::Text(large_text.clone().into_boxed_str());

    // Verify the cell holds the full string without truncation
    match &cell {
        CellValue::Text(s) => {
            assert_eq!(
                s.len(),
                size,
                "CellValue::Text truncated from {} to {}",
                size,
                s.len()
            );
        }
        _ => panic!("expected CellValue::Text"),
    }

    // Roundtrip through serde_json to verify serialization doesn't truncate
    let serialized = serde_json::to_vec(&cell).expect("serialize failed");
    let deserialized: CellValue =
        serde_json::from_slice(&serialized).expect("deserialize failed");

    match deserialized {
        CellValue::Text(s) => {
            assert_eq!(
                s.len(),
                size,
                "CellValue::Text truncated after serde roundtrip: {} -> {}",
                size,
                s.len()
            );
            assert_eq!(&*s, &*large_text.into_boxed_str());
        }
        _ => panic!("expected CellValue::Text after deserialization"),
    }
}

#[test]
fn stress_mixed_nulls_1m_rows() {
    let n = 1_000_000usize;

    // Deterministic pseudo-random null pattern using a simple LCG
    // (linear congruential generator) for reproducibility.
    let mut rng_state: u64 = 42; // fixed seed
    let mut nulls = vec![0u8; n];
    let mut values = vec![0.0f64; n];
    let mut expected_null_count = 0usize;

    for i in 0..n {
        // LCG: state = (a * state + c) mod m
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let is_null = (rng_state >> 33) & 1 == 1; // use high bit for ~50% distribution

        if is_null {
            nulls[i] = 1;
            expected_null_count += 1;
            // value stays 0.0 for null rows
        } else {
            nulls[i] = 0;
            values[i] = i as f64;
        }
    }

    // Verify roughly 50% null distribution (should be within 1% for 1M rows)
    let null_ratio = expected_null_count as f64 / n as f64;
    assert!(
        (0.49..=0.51).contains(&null_ratio),
        "null ratio {null_ratio} is outside expected 49-51% range"
    );

    let result = ColumnarResult {
        columns: vec![ColumnDef {
            name: "mixed".into(),
            data_type: "float8".into(),
        }],
        column_data: vec![ColumnStorage::Number {
            nulls: nulls.clone(),
            values: values.clone(),
        }],
        row_count: n as u64,
        execution_time_ms: 0,
        truncated: false,
    };

    let bytes = result.encode();

    // Verify encoding succeeded and size is reasonable
    assert!(
        bytes.len() > n * 8,
        "encoded size {} too small",
        bytes.len()
    );

    // Verify the null bitmap was written correctly by checking the encoded bytes.
    // Layout after header + column defs: type_tag(1) + nulls(n) + padding + values(n*8)
    //
    // Parse header to find where column data starts:
    // Header: 4 (col_count) + 8 (row_count) + 8 (exec_time) + 1 (truncated) + 4 (padding) = 25
    // Column defs: 2 (name_len) + 5 ("mixed") + 2 (type_len) + 6 ("float8") = 15
    let col_data_offset = 25 + 15;

    // type tag
    assert_eq!(bytes[col_data_offset], 0, "expected Number type tag");

    // null bitmap starts right after type tag
    let null_start = col_data_offset + 1;
    let mut encoded_null_count = 0usize;
    for i in 0..n {
        let encoded_null = bytes[null_start + i];
        assert_eq!(
            encoded_null, nulls[i],
            "null bitmap mismatch at row {i}: encoded={encoded_null}, expected={}",
            nulls[i]
        );
        if encoded_null == 1 {
            encoded_null_count += 1;
        }
    }

    assert_eq!(
        encoded_null_count, expected_null_count,
        "encoded null count {encoded_null_count} != expected {expected_null_count}"
    );
}
