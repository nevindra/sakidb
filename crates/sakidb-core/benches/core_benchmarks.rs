use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sakidb_core::driver::rows_to_columnar;
use sakidb_core::sql::{split_sql_statements, split_sql_statements_with, SqlSplitOptions};
use sakidb_core::types::*;

// ── Helpers ──

fn build_query_result(row_count: usize, col_count: usize) -> QueryResult {
    let columns: Vec<ColumnDef> = (0..col_count)
        .map(|c| match c % 4 {
            0 => ColumnDef {
                name: format!("int_col_{c}"),
                data_type: "integer".to_string(),
            },
            1 => ColumnDef {
                name: format!("text_col_{c}"),
                data_type: "text".to_string(),
            },
            2 => ColumnDef {
                name: format!("float_col_{c}"),
                data_type: "double precision".to_string(),
            },
            _ => ColumnDef {
                name: format!("bool_col_{c}"),
                data_type: "boolean".to_string(),
            },
        })
        .collect();

    let mut cells = Vec::with_capacity(row_count * col_count);
    for row in 0..row_count {
        for col in 0..col_count {
            // Sprinkle ~5% nulls
            if row % 20 == 0 && col % 3 == 0 {
                cells.push(CellValue::Null);
                continue;
            }
            match col % 4 {
                0 => cells.push(CellValue::Int(row as i64 * 1000 + col as i64)),
                1 => cells.push(CellValue::Text(
                    format!("row_{row}_col_{col}_value").into_boxed_str(),
                )),
                2 => cells.push(CellValue::Float(row as f64 * 3.14 + col as f64)),
                _ => cells.push(CellValue::Bool(row % 2 == 0)),
            }
        }
    }

    QueryResult {
        columns,
        cells,
        row_count: row_count as u64,
        execution_time_ms: 42,
        truncated: false,
    }
}

fn build_multi_query_result(row_count: usize, col_count: usize) -> MultiQueryResult {
    MultiQueryResult {
        results: vec![build_query_result(row_count, col_count)],
        total_execution_time_ms: 42,
    }
}

fn build_columnar_result(row_count: usize, col_count: usize) -> ColumnarResult {
    let multi = build_multi_query_result(row_count, col_count);
    let mut columnar = rows_to_columnar(multi);
    columnar.results.remove(0)
}

// ── Benchmarks ──

fn bench_columnar_encode_1k_rows(c: &mut Criterion) {
    c.bench_function("columnar_encode_1k_rows_10cols", |b| {
        b.iter_with_setup(
            || build_columnar_result(1_000, 10),
            |result| black_box(result.encode()),
        );
    });
}

fn bench_columnar_encode_100k_rows(c: &mut Criterion) {
    c.bench_function("columnar_encode_100k_rows_10cols", |b| {
        b.iter_with_setup(
            || build_columnar_result(100_000, 10),
            |result| black_box(result.encode()),
        );
    });
}

fn bench_columnar_decode_roundtrip(c: &mut Criterion) {
    c.bench_function("columnar_encode_decode_roundtrip_10k", |b| {
        b.iter_with_setup(
            || build_columnar_result(10_000, 10),
            |result| {
                let bytes = result.encode();
                black_box(bytes.len());
            },
        );
    });
}

fn bench_rows_to_columnar_10k(c: &mut Criterion) {
    c.bench_function("rows_to_columnar_10k_rows_10cols", |b| {
        b.iter_with_setup(
            || build_multi_query_result(10_000, 10),
            |multi| black_box(rows_to_columnar(multi)),
        );
    });
}

fn bench_columnar_memory_100k_rows(c: &mut Criterion) {
    c.bench_function("columnar_encode_size_100k_rows_10cols", |b| {
        b.iter_with_setup(
            || build_columnar_result(100_000, 10),
            |result| {
                let encoded = result.encode();
                black_box(encoded.len());
            },
        );
    });
}

fn bench_cellvalue_vs_serde_json(c: &mut Criterion) {
    let mut group = c.benchmark_group("cellvalue_vs_serde_json");

    group.bench_function("cellvalue_serialize_10k", |b| {
        let cells: Vec<CellValue> = (0..10_000)
            .map(|i| match i % 5 {
                0 => CellValue::Int(i as i64),
                1 => CellValue::Float(i as f64 * 1.23),
                2 => CellValue::Text(format!("text_value_{i}").into_boxed_str()),
                3 => CellValue::Bool(i % 2 == 0),
                _ => CellValue::Null,
            })
            .collect();
        b.iter(|| {
            let bytes = serde_json::to_vec(black_box(&cells)).unwrap();
            black_box(bytes.len());
        });
    });

    group.bench_function("serde_json_value_serialize_10k", |b| {
        let values: Vec<serde_json::Value> = (0..10_000)
            .map(|i| match i % 5 {
                0 => serde_json::Value::Number(serde_json::Number::from(i as i64)),
                1 => serde_json::json!(i as f64 * 1.23),
                2 => serde_json::Value::String(format!("text_value_{i}")),
                3 => serde_json::Value::Bool(i % 2 == 0),
                _ => serde_json::Value::Null,
            })
            .collect();
        b.iter(|| {
            let bytes = serde_json::to_vec(black_box(&values)).unwrap();
            black_box(bytes.len());
        });
    });

    group.finish();
}

fn bench_split_sql_small(c: &mut Criterion) {
    let sql = (0..10)
        .map(|i| format!("SELECT {i}"))
        .collect::<Vec<_>>()
        .join("; ");
    c.bench_function("split_sql_10_stmts", |b| {
        b.iter(|| black_box(split_sql_statements(black_box(&sql))));
    });
}

fn bench_split_sql_large(c: &mut Criterion) {
    let mut group = c.benchmark_group("split_sql_large");

    // 1000 simple statements
    let simple_sql = (0..1_000)
        .map(|i| format!("SELECT {i}"))
        .collect::<Vec<_>>()
        .join("; ");
    group.bench_function("split_sql_1000_simple_stmts", |b| {
        b.iter(|| black_box(split_sql_statements(black_box(&simple_sql))));
    });

    // 200 statements with strings, comments, and mixed content
    let complex_sql = (0..200)
        .map(|i| {
            format!(
                "-- comment {i}\nINSERT INTO t VALUES ({i}, 'it''s a test; not a delimiter', /* block ; comment */ 'done')"
            )
        })
        .collect::<Vec<_>>()
        .join(";\n");
    group.bench_function("split_sql_200_with_comments_strings", |b| {
        b.iter(|| black_box(split_sql_statements(black_box(&complex_sql))));
    });

    // 100 dollar-quoted statements (PostgreSQL)
    let dollar_sql = (0..100)
        .map(|i| {
            format!(
                "CREATE FUNCTION fn_{i}() RETURNS void AS $body$\nBEGIN\n  RAISE NOTICE 'hello; world';\nEND;\n$body$ LANGUAGE plpgsql"
            )
        })
        .collect::<Vec<_>>()
        .join(";\n");
    let opts = SqlSplitOptions {
        dollar_quoting: true,
    };
    group.bench_function("split_sql_100_dollar_quoted", |b| {
        b.iter(|| black_box(split_sql_statements_with(black_box(&dollar_sql), &opts)));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_columnar_encode_1k_rows,
    bench_columnar_encode_100k_rows,
    bench_columnar_decode_roundtrip,
    bench_rows_to_columnar_10k,
    bench_columnar_memory_100k_rows,
    bench_cellvalue_vs_serde_json,
    bench_split_sql_small,
    bench_split_sql_large,
);
criterion_main!(benches);
