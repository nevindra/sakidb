use std::collections::HashMap;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sakidb_core::types::*;
use sakidb_core::{Driver, SqlDriver};
use sakidb_oracle::OracleDriver;

// ── Helpers ──

/// TEST_ORACLE_URL format: username/password@host:port/service_name
fn get_test_config() -> Option<ConnectionConfig> {
    let raw = std::env::var("TEST_ORACLE_URL").ok()?;
    
    // Simple parsing for benchmark purposes
    let (userpass, hostinfo) = raw.split_once('@')?;
    let (username, password) = userpass.split_once('/')?;
    let (hostport, service) = hostinfo.split_once('/')?;
    let (host, port_str) = hostport.split_once(':').unwrap_or((hostport, "1521"));
    let port = port_str.parse::<u16>().unwrap_or(1521);
    
    Some(ConnectionConfig {
        engine: EngineType::Oracle,
        host: host.to_string(),
        port,
        database: service.to_string(),
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
                eprintln!("Skipping oracle benchmarks: TEST_ORACLE_URL not set");
                return;
            }
        }
    };
}

// ── Benchmarks ──

fn bench_execute_select_1k_rows(c: &mut Criterion) {
    let config = skip_if_no_db!();
    let rt = create_runtime();
    let driver = OracleDriver::new();

    let conn_id = rt.block_on(driver.connect(&config)).unwrap();

    // Setup: create table and insert 1K rows
    // Note: Oracle doesn't have DROP TABLE IF EXISTS
    let _ = rt.block_on(driver.execute(&conn_id, "DROP TABLE ora_bench_data PURGE"));
    
    rt.block_on(driver.execute(
        &conn_id,
        "CREATE TABLE ora_bench_data (
            id NUMBER PRIMARY KEY,
            name VARCHAR2(100) NOT NULL,
            value NUMBER,
            active NUMBER(1),
            data CLOB
        )",
    ))
    .unwrap();

    // Insert 1K rows
    for i in 0..1_000 {
        let insert_sql = format!(
            "INSERT INTO ora_bench_data (id, name, value, active, data) VALUES ({i}, 'name_{i}', {val}, {active}, 'data_payload_{i}')",
            val = i as f64 * 3.14,
            active = if i % 2 == 0 { 1 } else { 0 }
        );
        rt.block_on(driver.execute(&conn_id, &insert_sql)).unwrap();
    }

    c.bench_function("oracle_select_1k_rows", |b| {
        b.iter(|| {
            let result = rt
                .block_on(driver.execute(&conn_id, "SELECT * FROM ora_bench_data"))
                .unwrap();
            black_box(result.row_count);
        });
    });

    // Cleanup
    let _ = rt.block_on(driver.execute(&conn_id, "DROP TABLE ora_bench_data PURGE"));
    rt.block_on(driver.disconnect(&conn_id)).unwrap();
}

fn bench_execute_columnar_10k_rows(c: &mut Criterion) {
    let config = skip_if_no_db!();
    let rt = create_runtime();
    let driver = OracleDriver::new();

    let conn_id = rt.block_on(driver.connect(&config)).unwrap();

    let _ = rt.block_on(driver.execute(&conn_id, "DROP TABLE ora_bench_columnar PURGE"));
    rt.block_on(driver.execute(
        &conn_id,
        "CREATE TABLE ora_bench_columnar (
            id NUMBER PRIMARY KEY,
            name VARCHAR2(100) NOT NULL,
            value NUMBER,
            active NUMBER(1)
        )",
    ))
    .unwrap();

    // Insert 10K rows
    for i in 0..10_000 {
        let insert_sql = format!(
            "INSERT INTO ora_bench_columnar (id, name, value, active) VALUES ({i}, 'name_{i}', {val}, {active})",
            val = i as f64 * 2.71,
            active = if i % 2 == 0 { 1 } else { 0 }
        );
        rt.block_on(driver.execute(&conn_id, &insert_sql)).unwrap();
    }

    c.bench_function("oracle_columnar_10k_rows", |b| {
        b.iter(|| {
            let result = rt
                .block_on(
                    driver.execute_multi_columnar(&conn_id, "SELECT * FROM ora_bench_columnar"),
                )
                .unwrap();
            black_box(result.total_execution_time_ms);
        });
    });

    let _ = rt.block_on(driver.execute(&conn_id, "DROP TABLE ora_bench_columnar PURGE"));
    rt.block_on(driver.disconnect(&conn_id)).unwrap();
}

criterion_group!(
    benches,
    bench_execute_select_1k_rows,
    bench_execute_columnar_10k_rows,
);
criterion_main!(benches);
