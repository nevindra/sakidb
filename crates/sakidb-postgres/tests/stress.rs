#![cfg(feature = "stress")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use sakidb_core::types::*;
use sakidb_core::{Driver, Exporter, SqlDriver};
use sakidb_postgres::PostgresDriver;

fn get_test_url() -> Option<String> {
    std::env::var("TEST_DATABASE_URL").ok()
}

/// Parse a postgres connection URL into ConnectionConfig.
/// Expects format: postgres://user:pass@host:port/dbname
fn make_config(url: &str) -> ConnectionConfig {
    // Strip scheme
    let rest = url
        .strip_prefix("postgres://")
        .or_else(|| url.strip_prefix("postgresql://"))
        .expect("TEST_DATABASE_URL must start with postgres:// or postgresql://");

    // Split user_info@host_part/db
    let (user_info, rest) = rest.split_once('@').unwrap_or(("", rest));
    let (host_port, database) = rest.split_once('/').unwrap_or((rest, ""));

    let (username, password) = user_info.split_once(':').unwrap_or((user_info, ""));
    let (host, port_str) = host_port.split_once(':').unwrap_or((host_port, "5432"));
    let port: u16 = port_str.parse().unwrap_or(5432);

    ConnectionConfig {
        engine: EngineType::Postgres,
        host: host.to_string(),
        port,
        database: database.to_string(),
        username: username.to_string(),
        password: password.to_string(),
        ssl_mode: SslMode::Disable,
        options: std::collections::HashMap::new(),
    }
}

#[tokio::test]
async fn stress_postgres_connection_pool_exhaustion() {
    let url = match get_test_url() {
        Some(u) => u,
        None => {
            eprintln!("skipping: TEST_DATABASE_URL not set");
            return;
        }
    };

    let driver = PostgresDriver::new();
    let config = make_config(&url);

    let conn_id = driver
        .connect(&config)
        .await
        .expect("connect failed");

    // Execute many concurrent simple queries to stress the connection pool.
    // The pool should handle this gracefully — either queuing or returning errors,
    // but never panicking or deadlocking.
    let num_concurrent = 50;

    // Pre-allocate query strings so they outlive the futures that borrow them.
    let queries: Vec<String> = (0..num_concurrent).map(|i| format!("SELECT {i}")).collect();
    let join_handles: Vec<_> = queries
        .iter()
        .map(|q| driver.execute(&conn_id, q))
        .collect();

    let results: Vec<_> = futures::future::join_all(join_handles).await;

    // Count successes and failures — both are acceptable under pool exhaustion,
    // but we should get at least some successes and no panics.
    let successes = results.iter().filter(|r| r.is_ok()).count();
    let failures = results.iter().filter(|r| r.is_err()).count();

    eprintln!(
        "pool exhaustion: {successes} successes, {failures} failures out of {num_concurrent}"
    );

    // We expect at least some queries to succeed
    assert!(
        successes > 0,
        "all {num_concurrent} concurrent queries failed — pool is broken"
    );

    driver
        .disconnect(&conn_id)
        .await
        .expect("disconnect failed");
}

#[tokio::test]
async fn stress_export_csv_1m_rows() {
    let url = match get_test_url() {
        Some(u) => u,
        None => {
            eprintln!("skipping: TEST_DATABASE_URL not set");
            return;
        }
    };

    let driver = PostgresDriver::new();
    let config = make_config(&url);

    let conn_id = driver
        .connect(&config)
        .await
        .expect("connect failed");

    // Create a temp table with 1M rows using generate_series
    driver
        .execute(
            &conn_id,
            "CREATE TEMP TABLE stress_export (id int, val text)",
        )
        .await
        .expect("create table failed");

    driver
        .execute(
            &conn_id,
            "INSERT INTO stress_export SELECT g, 'row_' || g FROM generate_series(1, 1000000) g",
        )
        .await
        .expect("insert failed");

    // Export with streaming batches, counting total rows received
    let cancelled = AtomicBool::new(false);
    let row_count = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let row_count_clone = row_count.clone();

    let on_batch: Box<dyn Fn(&[ColumnDef], &[CellValue], u64) -> sakidb_core::error::Result<()> + Send + Sync> =
        Box::new(move |_cols, _cells, total| {
            row_count_clone.store(total, Ordering::Relaxed);
            Ok(())
        });

    let total = driver
        .export_stream(&conn_id, "SELECT * FROM stress_export", 10_000, &cancelled, &*on_batch)
        .await
        .expect("export failed");

    assert_eq!(
        total, 1_000_000,
        "export returned {total} rows, expected 1,000,000"
    );

    // Clean up
    driver
        .execute(&conn_id, "DROP TABLE IF EXISTS stress_export")
        .await
        .ok();

    driver
        .disconnect(&conn_id)
        .await
        .expect("disconnect failed");
}

#[tokio::test]
async fn stress_cancel_during_large_operation() {
    let url = match get_test_url() {
        Some(u) => u,
        None => {
            eprintln!("skipping: TEST_DATABASE_URL not set");
            return;
        }
    };

    let driver = PostgresDriver::new();
    let config = make_config(&url);

    let conn_id = driver
        .connect(&config)
        .await
        .expect("connect failed");

    // Create a temp table with 100K rows
    driver
        .execute(
            &conn_id,
            "CREATE TEMP TABLE stress_cancel (id int, val text)",
        )
        .await
        .expect("create table failed");

    driver
        .execute(
            &conn_id,
            "INSERT INTO stress_cancel SELECT g, repeat('x', 100) FROM generate_series(1, 100000) g",
        )
        .await
        .expect("insert failed");

    // Start an export and cancel it after receiving some rows
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancelled_for_callback = cancelled.clone();
    let rows_before_cancel = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let rows_clone = rows_before_cancel.clone();

    let on_batch: Box<dyn Fn(&[ColumnDef], &[CellValue], u64) -> sakidb_core::error::Result<()> + Send + Sync> =
        Box::new(move |_cols, _cells, total| {
            rows_clone.store(total, Ordering::Relaxed);
            // Cancel after receiving at least 1000 rows
            if total >= 1000 {
                cancelled_for_callback.store(true, Ordering::Relaxed);
            }
            Ok(())
        });

    let result = driver
        .export_stream(
            &conn_id,
            "SELECT * FROM stress_cancel",
            500,
            &cancelled,
            &*on_batch,
        )
        .await;

    let rows_received = rows_before_cancel.load(Ordering::Relaxed);

    // The export should either return a Cancelled error or complete with fewer rows
    match result {
        Err(sakidb_core::SakiError::Cancelled) => {
            eprintln!("export cancelled after {rows_received} rows (expected)");
            assert!(
                rows_received >= 1000,
                "cancellation triggered too early: only {rows_received} rows"
            );
            assert!(
                rows_received < 100_000,
                "cancellation didn't stop export: got all {rows_received} rows"
            );
        }
        Ok(total) => {
            // Some drivers may complete before cancellation takes effect
            eprintln!("export completed with {total} rows before cancellation took effect");
        }
        Err(e) => {
            // QueryFailed with cancellation message is also acceptable
            let msg = format!("{e}");
            assert!(
                msg.contains("cancel") || msg.contains("Cancel"),
                "unexpected error (not cancellation): {e}"
            );
        }
    }

    // Clean up
    driver
        .execute(&conn_id, "DROP TABLE IF EXISTS stress_cancel")
        .await
        .ok();

    driver
        .disconnect(&conn_id)
        .await
        .expect("disconnect failed");
}
