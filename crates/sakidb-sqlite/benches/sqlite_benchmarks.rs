use std::collections::HashMap;
use std::sync::atomic::AtomicBool;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sakidb_core::types::*;
use sakidb_core::{Driver, Restorer, SqlDriver};
use sakidb_sqlite::SqliteDriver;

// ── Helpers ──

fn make_config(path: &str) -> ConnectionConfig {
    ConnectionConfig {
        engine: EngineType::Sqlite,
        host: String::new(),
        port: 0,
        database: path.to_string(),
        username: String::new(),
        password: String::new(),
        ssl_mode: SslMode::Disable,
        options: HashMap::new(),
    }
}

fn create_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
}

/// Create a temp DB with a populated table. Returns (temp_dir, conn_id, driver).
/// The temp_dir must stay alive for the duration of the benchmark.
fn setup_db(
    rt: &tokio::runtime::Runtime,
    row_count: usize,
) -> (tempfile::TempDir, ConnectionId, SqliteDriver) {
    let tmp = tempfile::TempDir::new().unwrap();
    let db_path = tmp.path().join("bench.db").to_string_lossy().to_string();
    let driver = SqliteDriver::new();

    let conn_id = rt.block_on(driver.connect(&make_config(&db_path))).unwrap();

    // Create table with mixed column types
    rt.block_on(driver.execute(
        &conn_id,
        "CREATE TABLE bench_data (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            value REAL,
            active INTEGER,
            data TEXT
        )",
    ))
    .unwrap();

    // Batch insert rows
    let batch_size = 500;
    for batch_start in (0..row_count).step_by(batch_size) {
        let batch_end = (batch_start + batch_size).min(row_count);
        let mut sql = String::from("INSERT INTO bench_data (id, name, value, active, data) VALUES ");
        for i in batch_start..batch_end {
            if i > batch_start {
                sql.push_str(", ");
            }
            sql.push_str(&format!(
                "({i}, 'name_{i}_with_some_extra_text', {val}, {active}, 'data_payload_{i}_lorem_ipsum_dolor_sit_amet')",
                val = i as f64 * 3.14,
                active = if i % 2 == 0 { 1 } else { 0 }
            ));
        }
        rt.block_on(driver.execute(&conn_id, &sql)).unwrap();
    }

    (tmp, conn_id, driver)
}

// ── Benchmarks ──

fn bench_execute_select_1k_rows(c: &mut Criterion) {
    let rt = create_runtime();
    let (_tmp, conn_id, driver) = setup_db(&rt, 1_000);

    c.bench_function("sqlite_select_1k_rows", |b| {
        b.iter(|| {
            let result = rt
                .block_on(driver.execute(&conn_id, "SELECT * FROM bench_data"))
                .unwrap();
            black_box(result.row_count);
        });
    });

    rt.block_on(driver.disconnect(&conn_id)).unwrap();
}

fn bench_execute_columnar_100k_rows(c: &mut Criterion) {
    let rt = create_runtime();
    let (_tmp, conn_id, driver) = setup_db(&rt, 100_000);

    c.bench_function("sqlite_columnar_100k_rows", |b| {
        b.iter(|| {
            let result = rt
                .block_on(driver.execute_multi_columnar(&conn_id, "SELECT * FROM bench_data"))
                .unwrap();
            black_box(result.total_execution_time_ms);
        });
    });

    rt.block_on(driver.disconnect(&conn_id)).unwrap();
}

fn bench_bulk_insert_10k_rows(c: &mut Criterion) {
    let rt = create_runtime();

    c.bench_function("sqlite_bulk_insert_10k_rows", |b| {
        b.iter_with_setup(
            || {
                // Fresh DB for each iteration to avoid table bloat
                let tmp = tempfile::TempDir::new().unwrap();
                let db_path = tmp.path().join("bench.db").to_string_lossy().to_string();
                let driver = SqliteDriver::new();
                let conn_id = rt.block_on(driver.connect(&make_config(&db_path))).unwrap();
                rt.block_on(driver.execute(
                    &conn_id,
                    "CREATE TABLE bulk_test (id INTEGER PRIMARY KEY, name TEXT, value REAL)",
                ))
                .unwrap();
                (tmp, conn_id, driver)
            },
            |(_tmp, conn_id, driver)| {
                let batch_size = 500;
                for batch_start in (0..10_000).step_by(batch_size) {
                    let batch_end = batch_start + batch_size;
                    let mut sql = String::from(
                        "INSERT INTO bulk_test (id, name, value) VALUES ",
                    );
                    for i in batch_start..batch_end {
                        if i > batch_start {
                            sql.push_str(", ");
                        }
                        sql.push_str(&format!(
                            "({i}, 'name_{i}', {val})",
                            val = i as f64 * 2.71
                        ));
                    }
                    rt.block_on(driver.execute(&conn_id, &sql)).unwrap();
                }
                rt.block_on(driver.disconnect(&conn_id)).unwrap();
            },
        );
    });
}

fn bench_restore_1k_statements(c: &mut Criterion) {
    let rt = create_runtime();

    // Generate a SQL file with 1K INSERT statements
    let sql_content = {
        let mut s = String::from(
            "CREATE TABLE IF NOT EXISTS restore_test (id INTEGER PRIMARY KEY, name TEXT, value REAL);\n",
        );
        for i in 0..1_000 {
            s.push_str(&format!(
                "INSERT INTO restore_test VALUES ({i}, 'name_{i}', {val});\n",
                val = i as f64 * 1.5
            ));
        }
        s
    };

    c.bench_function("sqlite_restore_1k_stmts", |b| {
        b.iter_with_setup(
            || {
                // Fresh DB and SQL file for each iteration
                let tmp = tempfile::TempDir::new().unwrap();
                let db_path = tmp.path().join("bench.db").to_string_lossy().to_string();
                let sql_path = tmp.path().join("restore.sql");
                std::fs::write(&sql_path, &sql_content).unwrap();

                let driver = SqliteDriver::new();
                let conn_id = rt.block_on(driver.connect(&make_config(&db_path))).unwrap();
                (tmp, conn_id, driver, sql_path)
            },
            |(_tmp, conn_id, driver, sql_path)| {
                let cancelled = AtomicBool::new(false);
                let options = RestoreOptions {
                    schema: None,
                    continue_on_error: false,
                };
                let progress = rt
                    .block_on(driver.restore(
                        &conn_id,
                        sql_path.to_str().unwrap(),
                        &options,
                        &cancelled,
                        Box::new(|_| {}),
                    ))
                    .unwrap();
                black_box(progress.statements_executed);
                rt.block_on(driver.disconnect(&conn_id)).unwrap();
            },
        );
    });
}

criterion_group!(
    benches,
    bench_execute_select_1k_rows,
    bench_execute_columnar_100k_rows,
    bench_bulk_insert_10k_rows,
    bench_restore_1k_statements,
);
criterion_main!(benches);
