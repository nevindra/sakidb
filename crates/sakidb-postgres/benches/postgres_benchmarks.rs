use std::collections::HashMap;
use std::sync::atomic::AtomicBool;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sakidb_core::types::*;
use sakidb_core::{Driver, Restorer, SqlDriver};
use sakidb_postgres::PostgresDriver;

// ── Helpers ──

/// Parse TEST_DATABASE_URL (postgres://user:pass@host:port/dbname) without
/// pulling in the `url` crate.
fn get_test_config() -> Option<ConnectionConfig> {
    let raw = std::env::var("TEST_DATABASE_URL").ok()?;
    // Strip scheme
    let rest = raw.strip_prefix("postgres://").or_else(|| raw.strip_prefix("postgresql://"))?;
    // Split userinfo @ hostinfo / dbname
    let (userinfo, rest) = rest.split_once('@')?;
    let (host_port, database) = rest.split_once('/')?;
    let (username, password) = userinfo.split_once(':').unwrap_or((userinfo, ""));
    let (host, port_str) = host_port.split_once(':').unwrap_or((host_port, "5432"));
    let port = port_str.parse::<u16>().unwrap_or(5432);
    Some(ConnectionConfig {
        engine: EngineType::Postgres,
        host: host.to_string(),
        port,
        database: database.to_string(),
        username: username.to_string(),
        password: password.to_string(),
        ssl_mode: SslMode::Disable,
        options: HashMap::new(),
    })
}

fn create_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
}

macro_rules! skip_if_no_db {
    () => {
        match get_test_config() {
            Some(config) => config,
            None => {
                eprintln!("Skipping postgres benchmarks: TEST_DATABASE_URL not set");
                return;
            }
        }
    };
}

// ── Benchmarks ──

fn bench_execute_select_1k_rows(c: &mut Criterion) {
    let config = skip_if_no_db!();
    let rt = create_runtime();
    let driver = PostgresDriver::new();

    let conn_id = rt.block_on(driver.connect(&config)).unwrap();

    // Setup: create table and insert 1K rows
    rt.block_on(driver.execute(
        &conn_id,
        "DROP TABLE IF EXISTS pg_bench_data",
    ))
    .unwrap();
    rt.block_on(driver.execute(
        &conn_id,
        "CREATE TABLE pg_bench_data (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            value DOUBLE PRECISION,
            active BOOLEAN,
            data TEXT
        )",
    ))
    .unwrap();

    let insert_sql: String = (0..1_000)
        .map(|i| {
            format!(
                "INSERT INTO pg_bench_data VALUES ({i}, 'name_{i}_with_extra_text', {val}, {active}, 'data_payload_{i}_lorem_ipsum')",
                val = i as f64 * 3.14,
                active = if i % 2 == 0 { "true" } else { "false" }
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    rt.block_on(driver.execute_multi(&conn_id, &insert_sql))
        .unwrap();

    c.bench_function("postgres_select_1k_rows", |b| {
        b.iter(|| {
            let result = rt
                .block_on(driver.execute(&conn_id, "SELECT * FROM pg_bench_data"))
                .unwrap();
            black_box(result.row_count);
        });
    });

    // Cleanup
    rt.block_on(driver.execute(&conn_id, "DROP TABLE IF EXISTS pg_bench_data"))
        .unwrap();
    rt.block_on(driver.disconnect(&conn_id)).unwrap();
}

fn bench_execute_columnar_10k_rows(c: &mut Criterion) {
    let config = skip_if_no_db!();
    let rt = create_runtime();
    let driver = PostgresDriver::new();

    let conn_id = rt.block_on(driver.connect(&config)).unwrap();

    rt.block_on(driver.execute(
        &conn_id,
        "DROP TABLE IF EXISTS pg_bench_columnar",
    ))
    .unwrap();
    rt.block_on(driver.execute(
        &conn_id,
        "CREATE TABLE pg_bench_columnar (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            value DOUBLE PRECISION,
            active BOOLEAN
        )",
    ))
    .unwrap();

    // Insert 10K rows in batches
    for batch_start in (0..10_000).step_by(500) {
        let batch_end = batch_start + 500;
        let insert_sql: String = (batch_start..batch_end)
            .map(|i| {
                format!(
                    "INSERT INTO pg_bench_columnar VALUES ({i}, 'name_{i}', {val}, {active})",
                    val = i as f64 * 2.71,
                    active = if i % 2 == 0 { "true" } else { "false" }
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        rt.block_on(driver.execute_multi(&conn_id, &insert_sql))
            .unwrap();
    }

    c.bench_function("postgres_columnar_10k_rows", |b| {
        b.iter(|| {
            let result = rt
                .block_on(
                    driver.execute_multi_columnar(&conn_id, "SELECT * FROM pg_bench_columnar"),
                )
                .unwrap();
            black_box(result.total_execution_time_ms);
        });
    });

    rt.block_on(driver.execute(
        &conn_id,
        "DROP TABLE IF EXISTS pg_bench_columnar",
    ))
    .unwrap();
    rt.block_on(driver.disconnect(&conn_id)).unwrap();
}

fn bench_bulk_insert_1k_rows(c: &mut Criterion) {
    let config = skip_if_no_db!();
    let rt = create_runtime();
    let driver = PostgresDriver::new();

    let conn_id = rt.block_on(driver.connect(&config)).unwrap();

    c.bench_function("postgres_bulk_insert_1k_rows", |b| {
        b.iter_with_setup(
            || {
                rt.block_on(driver.execute(
                    &conn_id,
                    "DROP TABLE IF EXISTS pg_bench_insert",
                ))
                .unwrap();
                rt.block_on(driver.execute(
                    &conn_id,
                    "CREATE TABLE pg_bench_insert (id INTEGER PRIMARY KEY, name TEXT, value DOUBLE PRECISION)",
                ))
                .unwrap();
            },
            |()| {
                for batch_start in (0..1_000).step_by(100) {
                    let batch_end = batch_start + 100;
                    let insert_sql: String = (batch_start..batch_end)
                        .map(|i| {
                            format!(
                                "INSERT INTO pg_bench_insert VALUES ({i}, 'name_{i}', {val})",
                                val = i as f64 * 2.71
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("; ");
                    rt.block_on(driver.execute_multi(&conn_id, &insert_sql)).unwrap();
                }
            },
        );
    });

    rt.block_on(driver.execute(
        &conn_id,
        "DROP TABLE IF EXISTS pg_bench_insert",
    ))
    .unwrap();
    rt.block_on(driver.disconnect(&conn_id)).unwrap();
}

fn bench_restore_500_statements(c: &mut Criterion) {
    let config = skip_if_no_db!();
    let rt = create_runtime();
    let driver = PostgresDriver::new();

    let conn_id = rt.block_on(driver.connect(&config)).unwrap();

    // Generate a SQL file with 500 INSERT statements
    let sql_content = {
        let mut s = String::from(
            "DROP TABLE IF EXISTS pg_bench_restore;\n\
             CREATE TABLE pg_bench_restore (id INTEGER PRIMARY KEY, name TEXT, value DOUBLE PRECISION);\n",
        );
        for i in 0..500 {
            s.push_str(&format!(
                "INSERT INTO pg_bench_restore VALUES ({i}, 'name_{i}', {val});\n",
                val = i as f64 * 1.5
            ));
        }
        s
    };

    c.bench_function("postgres_restore_500_stmts", |b| {
        b.iter_with_setup(
            || {
                let tmp = tempfile::TempDir::new().unwrap();
                let sql_path = tmp.path().join("restore.sql");
                std::fs::write(&sql_path, &sql_content).unwrap();
                // Drop table before each iteration
                rt.block_on(driver.execute(
                    &conn_id,
                    "DROP TABLE IF EXISTS pg_bench_restore",
                ))
                .unwrap();
                (tmp, sql_path)
            },
            |(_tmp, sql_path)| {
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
            },
        );
    });

    rt.block_on(driver.execute(
        &conn_id,
        "DROP TABLE IF EXISTS pg_bench_restore",
    ))
    .unwrap();
    rt.block_on(driver.disconnect(&conn_id)).unwrap();
}

criterion_group!(
    benches,
    bench_execute_select_1k_rows,
    bench_execute_columnar_10k_rows,
    bench_bulk_insert_1k_rows,
    bench_restore_500_statements,
);
criterion_main!(benches);
