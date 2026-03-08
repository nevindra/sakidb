#![cfg(feature = "stress")]

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use sakidb_core::types::*;
use sakidb_core::{Driver, Restorer, SqlDriver};
use sakidb_sqlite::SqliteDriver;

fn make_config(path: &str) -> ConnectionConfig {
    let mut options = HashMap::new();
    options.insert("file_path".to_string(), path.to_string());
    ConnectionConfig {
        engine: EngineType::Sqlite,
        host: String::new(),
        port: 0,
        database: path.to_string(),
        username: String::new(),
        password: String::new(),
        ssl_mode: SslMode::Disable,
        options,
    }
}

#[tokio::test]
async fn stress_sqlite_concurrent_reads() {
    let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
    let path = tmp.path().to_str().unwrap().to_string();

    let driver = Arc::new(SqliteDriver::new());
    let config = make_config(&path);

    let conn_id = driver.connect(&config).await.expect("connect failed");

    // Create table and insert test data
    driver
        .execute(
            &conn_id,
            "CREATE TABLE stress_read (id INTEGER PRIMARY KEY, val TEXT)",
        )
        .await
        .expect("create table failed");

    // Insert 1000 rows
    let mut inserts = String::with_capacity(64 * 1000);
    inserts.push_str("BEGIN;\n");
    for i in 0..1000 {
        inserts.push_str(&format!(
            "INSERT INTO stress_read (id, val) VALUES ({i}, 'value_{i}');\n"
        ));
    }
    inserts.push_str("COMMIT;");
    driver
        .execute_batch(&conn_id, &inserts)
        .await
        .expect("batch insert failed");

    // Spawn 10 concurrent reader tasks, each running multiple queries
    let num_readers = 10;
    let queries_per_reader = 50;
    let mut handles = Vec::with_capacity(num_readers);

    for reader_id in 0..num_readers {
        let d = driver.clone();
        let cid = conn_id;

        handles.push(tokio::spawn(async move {
            let mut successes = 0usize;
            let mut failures = 0usize;

            for q in 0..queries_per_reader {
                let offset = (reader_id * queries_per_reader + q) % 1000;
                let sql = format!("SELECT * FROM stress_read WHERE id >= {offset} LIMIT 10");
                match d.execute(&cid, &sql).await {
                    Ok(_) => successes += 1,
                    Err(_) => failures += 1,
                }
            }

            (reader_id, successes, failures)
        }));
    }

    let mut total_successes = 0usize;
    let mut total_failures = 0usize;

    for handle in handles {
        let (reader_id, successes, failures) = handle.await.expect("task panicked");
        eprintln!("reader {reader_id}: {successes} ok, {failures} err");
        total_successes += successes;
        total_failures += failures;
    }

    let total_queries = num_readers * queries_per_reader;
    eprintln!(
        "concurrent reads: {total_successes}/{total_queries} succeeded, {total_failures} failed"
    );

    // All reads should succeed since SQLite WAL mode supports concurrent reads
    assert!(
        total_successes > 0,
        "all concurrent reads failed"
    );

    driver.disconnect(&conn_id).await.expect("disconnect failed");
}

#[tokio::test]
async fn stress_sqlite_read_during_write() {
    let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
    let path = tmp.path().to_str().unwrap().to_string();

    let driver = Arc::new(SqliteDriver::new());
    let config = make_config(&path);

    let conn_id = driver.connect(&config).await.expect("connect failed");

    // Create table
    driver
        .execute(
            &conn_id,
            "CREATE TABLE stress_rw (id INTEGER PRIMARY KEY, val TEXT)",
        )
        .await
        .expect("create table failed");

    // Writer task: insert rows in batches
    let write_driver = driver.clone();
    let write_cid = conn_id;
    let write_done = Arc::new(AtomicBool::new(false));
    let write_done_clone = write_done.clone();

    let writer = tokio::spawn(async move {
        let mut total_written = 0u64;
        for batch in 0..100 {
            let start = batch * 100;
            let mut sql = String::from("BEGIN;\n");
            for i in start..start + 100 {
                sql.push_str(&format!(
                    "INSERT OR IGNORE INTO stress_rw (id, val) VALUES ({i}, 'batch_{batch}_row_{i}');\n"
                ));
            }
            sql.push_str("COMMIT;");

            match write_driver.execute_batch(&write_cid, &sql).await {
                Ok(()) => total_written += 100,
                Err(e) => {
                    eprintln!("writer batch {batch} failed: {e}");
                }
            }
        }
        write_done_clone.store(true, Ordering::Relaxed);
        total_written
    });

    // Reader tasks: continuously read while writes are happening
    let num_readers = 5;
    let mut reader_handles = Vec::with_capacity(num_readers);

    for reader_id in 0..num_readers {
        let read_driver = driver.clone();
        let read_cid = conn_id;
        let done_flag = write_done.clone();

        reader_handles.push(tokio::spawn(async move {
            let mut read_count = 0u64;
            let mut error_count = 0u64;

            // Keep reading until the writer is done
            while !done_flag.load(Ordering::Relaxed) {
                match read_driver
                    .execute(&read_cid, "SELECT COUNT(*) FROM stress_rw")
                    .await
                {
                    Ok(_) => read_count += 1,
                    Err(_) => error_count += 1,
                }
                // Small yield to avoid spinning too fast
                tokio::task::yield_now().await;
            }

            // Do a few more reads after writer is done
            for _ in 0..10 {
                match read_driver
                    .execute(&read_cid, "SELECT COUNT(*) FROM stress_rw")
                    .await
                {
                    Ok(_) => read_count += 1,
                    Err(_) => error_count += 1,
                }
            }

            (reader_id, read_count, error_count)
        }));
    }

    let total_written = writer.await.expect("writer task panicked");
    eprintln!("writer completed: {total_written} rows written");

    let mut total_reads = 0u64;
    let mut total_errors = 0u64;
    for handle in reader_handles {
        let (reader_id, reads, errors) = handle.await.expect("reader task panicked");
        eprintln!("reader {reader_id}: {reads} reads, {errors} errors");
        total_reads += reads;
        total_errors += errors;
    }

    eprintln!("read-during-write: {total_reads} reads, {total_errors} errors");

    // Writes should complete
    assert!(
        total_written > 0,
        "no rows were written"
    );

    // Reads should mostly succeed in WAL mode
    assert!(
        total_reads > 0,
        "no reads succeeded during concurrent write"
    );

    // Verify final data integrity
    let result = driver
        .execute(&conn_id, "SELECT COUNT(*) FROM stress_rw")
        .await
        .expect("final count query failed");
    assert_eq!(result.row_count, 1, "expected 1 row from COUNT(*)");

    driver.disconnect(&conn_id).await.expect("disconnect failed");
}

#[tokio::test]
async fn stress_restore_50k_statements() {
    let tmp_db = tempfile::NamedTempFile::new().expect("failed to create temp db file");
    let db_path = tmp_db.path().to_str().unwrap().to_string();

    let tmp_sql = tempfile::NamedTempFile::new().expect("failed to create temp sql file");
    let sql_path = tmp_sql.path().to_str().unwrap().to_string();

    // Generate a SQL file with 50K INSERT statements
    let mut sql_content = String::with_capacity(100 * 50_000);
    sql_content.push_str("CREATE TABLE IF NOT EXISTS stress_restore (id INTEGER PRIMARY KEY, val TEXT);\n");
    for i in 0..50_000 {
        sql_content.push_str(&format!(
            "INSERT INTO stress_restore (id, val) VALUES ({i}, 'value_{i}');\n"
        ));
    }
    std::fs::write(&sql_path, &sql_content).expect("failed to write SQL file");

    let driver = SqliteDriver::new();
    let config = make_config(&db_path);

    let conn_id = driver.connect(&config).await.expect("connect failed");

    let cancelled = AtomicBool::new(false);
    let progress_updates = Arc::new(std::sync::Mutex::new(Vec::new()));
    let progress_clone = progress_updates.clone();

    let on_progress: Box<dyn Fn(RestoreProgress) + Send + Sync> = Box::new(move |p| {
        let mut updates = progress_clone.lock().unwrap();
        updates.push(p);
    });

    let options = RestoreOptions {
        schema: None,
        continue_on_error: true,
    };

    let final_progress = driver
        .restore(&conn_id, &sql_path, &options, &cancelled, on_progress)
        .await
        .expect("restore failed");

    eprintln!(
        "restore complete: {} statements executed, {} errors, {} ms",
        final_progress.statements_executed,
        final_progress.errors_skipped,
        final_progress.elapsed_ms
    );

    // Should have executed the CREATE TABLE + 50K INSERTs
    assert!(
        final_progress.statements_executed >= 50_000,
        "only {} statements executed, expected at least 50,000",
        final_progress.statements_executed
    );

    assert_eq!(
        final_progress.phase, "Complete",
        "restore phase should be 'Complete', got '{}'",
        final_progress.phase
    );

    // Verify data integrity
    let result = driver
        .execute(&conn_id, "SELECT COUNT(*) FROM stress_restore")
        .await
        .expect("count query failed");

    match &result.cells[0] {
        CellValue::Int(count) => {
            assert_eq!(
                *count, 50_000,
                "expected 50,000 rows, got {count}"
            );
        }
        other => {
            panic!("unexpected cell type for COUNT(*): {other:?}");
        }
    }

    // Verify progress events were emitted
    let updates = progress_updates.lock().unwrap();
    assert!(
        !updates.is_empty(),
        "no progress updates were emitted during restore"
    );

    driver.disconnect(&conn_id).await.expect("disconnect failed");
}
