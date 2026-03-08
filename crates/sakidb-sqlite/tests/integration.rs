#![cfg(feature = "integration")]

use std::collections::HashMap;
use std::io::Write;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use sakidb_core::types::*;
use sakidb_core::{Driver, Exporter, Introspector, Restorer, SakiError, SqlDriver};
use sakidb_sqlite::SqliteDriver;
use tempfile::NamedTempFile;

/// Build a ConnectionConfig pointing at the given file path.
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

/// Create a temp file and connect. Returns (driver, conn_id, temp_file).
/// The temp_file is kept alive so the file is not deleted until the test finishes.
async fn setup() -> (SqliteDriver, ConnectionId, NamedTempFile) {
    let tmp = NamedTempFile::new().expect("failed to create temp file");
    let path = tmp.path().to_str().unwrap().to_string();
    let driver = SqliteDriver::new();
    let config = make_config(&path);
    let conn_id = driver.connect(&config).await.expect("connect failed");
    (driver, conn_id, tmp)
}

// ── Test 1: connect_and_disconnect ──

#[tokio::test]
async fn connect_and_disconnect() {
    let tmp = NamedTempFile::new().expect("failed to create temp file");
    let path = tmp.path().to_str().unwrap().to_string();
    let driver = SqliteDriver::new();
    let config = make_config(&path);

    let conn_id = driver.connect(&config).await.expect("connect failed");
    driver
        .disconnect(&conn_id)
        .await
        .expect("disconnect failed");
}

// ── Test 2: execute_select ──

#[tokio::test]
async fn execute_select() {
    let (driver, conn_id, _tmp) = setup().await;

    // Create table and insert data
    driver
        .execute_batch(
            &conn_id,
            "CREATE TABLE test_exec (id INTEGER PRIMARY KEY, name TEXT);
             INSERT INTO test_exec VALUES (1, 'hello');
             INSERT INTO test_exec VALUES (2, 'world')",
        )
        .await
        .expect("setup failed");

    let result = driver
        .execute(&conn_id, "SELECT id, name FROM test_exec ORDER BY id")
        .await
        .expect("execute failed");

    assert_eq!(result.columns.len(), 2);
    assert_eq!(result.columns[0].name, "id");
    assert_eq!(result.columns[1].name, "name");
    assert_eq!(result.row_count, 2);

    // Row 0: id=1, name=hello
    match &result.cells[0] {
        CellValue::Int(n) => assert_eq!(*n, 1),
        other => panic!("expected Int(1), got {:?}", other),
    }
    match &result.cells[1] {
        CellValue::Text(s) => assert_eq!(&**s, "hello"),
        other => panic!("expected Text(hello), got {:?}", other),
    }

    let _ = driver.disconnect(&conn_id).await;
}

// ── Test 3: execute_multi_statement ──

#[tokio::test]
async fn execute_multi_statement() {
    let (driver, conn_id, _tmp) = setup().await;

    let result = driver
        .execute_multi(&conn_id, "SELECT 1 AS a; SELECT 2 AS b, 3 AS c")
        .await
        .expect("execute_multi failed");

    assert_eq!(result.results.len(), 2);
    assert_eq!(result.results[0].columns.len(), 1);
    assert_eq!(result.results[0].columns[0].name, "a");
    assert_eq!(result.results[1].columns.len(), 2);

    match &result.results[0].cells[0] {
        CellValue::Int(n) => assert_eq!(*n, 1),
        other => panic!("expected Int(1), got {:?}", other),
    }

    let _ = driver.disconnect(&conn_id).await;
}

// ── Test 4: execute_columnar ──

#[tokio::test]
async fn execute_columnar() {
    let (driver, conn_id, _tmp) = setup().await;

    driver
        .execute_batch(
            &conn_id,
            "CREATE TABLE col_test (num INTEGER, val TEXT);
             INSERT INTO col_test VALUES (42, 'text')",
        )
        .await
        .expect("setup failed");

    let result = driver
        .execute_multi_columnar(&conn_id, "SELECT num, val FROM col_test")
        .await
        .expect("execute_multi_columnar failed");

    assert_eq!(result.results.len(), 1);
    let cr = &result.results[0];
    assert_eq!(cr.columns.len(), 2);
    assert_eq!(cr.row_count, 1);

    match &cr.column_data[0] {
        ColumnStorage::Number { values, .. } => assert_eq!(values[0] as i64, 42),
        other => panic!("expected Number column, got {:?}", other),
    }
    match &cr.column_data[1] {
        ColumnStorage::Text { data, offsets, .. } => {
            let s = std::str::from_utf8(&data[offsets[0] as usize..offsets[1] as usize]).unwrap();
            assert_eq!(s, "text");
        }
        other => panic!("expected Text column, got {:?}", other),
    }

    let _ = driver.disconnect(&conn_id).await;
}

// ── Test 5: execute_paged_and_batch ──

#[tokio::test]
async fn execute_paged_and_batch() {
    let (driver, conn_id, _tmp) = setup().await;

    // Create table and batch insert 100 rows
    let mut sql = String::from("CREATE TABLE paged_test (id INTEGER);");
    for i in 1..=100 {
        sql.push_str(&format!(" INSERT INTO paged_test VALUES ({i});"));
    }
    driver
        .execute_batch(&conn_id, &sql)
        .await
        .expect("batch insert failed");

    // Page 0 (first 25 rows, 0-based paging)
    let page0 = driver
        .execute_paged(&conn_id, "SELECT id FROM paged_test ORDER BY id", 0, 25)
        .await
        .expect("execute_paged page 0 failed");

    assert_eq!(page0.row_count, 25);
    assert_eq!(page0.page, 0);
    assert_eq!(page0.page_size, 25);

    match &page0.cells[0] {
        CellValue::Int(n) => assert_eq!(*n, 1),
        other => panic!("expected Int(1), got {:?}", other),
    }

    // Page 2 (rows 51-75, 0-based: offset = 2*25 = 50)
    let page2 = driver
        .execute_paged(&conn_id, "SELECT id FROM paged_test ORDER BY id", 2, 25)
        .await
        .expect("execute_paged page 2 failed");

    assert_eq!(page2.row_count, 25);
    match &page2.cells[0] {
        CellValue::Int(n) => assert_eq!(*n, 51),
        other => panic!("expected Int(51), got {:?}", other),
    }

    let _ = driver.disconnect(&conn_id).await;
}

// ── Test 6: list_tables_after_create ──

#[tokio::test]
async fn list_tables_after_create() {
    let (driver, conn_id, _tmp) = setup().await;

    driver
        .execute_batch(
            &conn_id,
            "CREATE TABLE my_table (id INTEGER PRIMARY KEY, name TEXT)",
        )
        .await
        .expect("create table failed");

    let tables = driver
        .list_tables(&conn_id, "main")
        .await
        .expect("list_tables failed");

    let found = tables.iter().any(|t| t.name == "my_table");
    assert!(found, "my_table not found in {:?}", tables);

    let _ = driver.disconnect(&conn_id).await;
}

// ── Test 7: list_columns_types ──

#[tokio::test]
async fn list_columns_types() {
    let (driver, conn_id, _tmp) = setup().await;

    driver
        .execute_batch(
            &conn_id,
            "CREATE TABLE typed_cols (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                score REAL,
                data BLOB
            )",
        )
        .await
        .expect("create table failed");

    let columns = driver
        .list_columns(&conn_id, "main", "typed_cols")
        .await
        .expect("list_columns failed");

    assert_eq!(columns.len(), 4);

    let id_col = columns.iter().find(|c| c.name == "id").unwrap();
    assert!(id_col.is_primary_key);

    let name_col = columns.iter().find(|c| c.name == "name").unwrap();
    assert!(!name_col.is_nullable);

    let score_col = columns.iter().find(|c| c.name == "score").unwrap();
    assert!(score_col.is_nullable);
    assert!(
        score_col.data_type.to_uppercase().contains("REAL"),
        "expected REAL type, got {}",
        score_col.data_type
    );

    let data_col = columns.iter().find(|c| c.name == "data").unwrap();
    assert!(
        data_col.data_type.to_uppercase().contains("BLOB"),
        "expected BLOB type, got {}",
        data_col.data_type
    );

    let _ = driver.disconnect(&conn_id).await;
}

// ── Test 8: introspect_full_schema ──

#[tokio::test]
async fn introspect_full_schema() {
    let (driver, conn_id, _tmp) = setup().await;

    driver
        .execute_batch(
            &conn_id,
            "CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT UNIQUE NOT NULL);
             CREATE TABLE posts (
                 id INTEGER PRIMARY KEY,
                 user_id INTEGER REFERENCES users(id),
                 title TEXT NOT NULL
             );
             CREATE VIEW user_emails AS SELECT id, email FROM users;
             CREATE INDEX idx_posts_user ON posts (user_id);
             CREATE TRIGGER trg_posts AFTER INSERT ON posts BEGIN SELECT 1; END",
        )
        .await
        .expect("setup objects failed");

    // Tables
    let tables = driver.list_tables(&conn_id, "main").await.unwrap();
    assert!(tables.iter().any(|t| t.name == "users"));
    assert!(tables.iter().any(|t| t.name == "posts"));

    // Views
    let views = driver.list_views(&conn_id, "main").await.unwrap();
    assert!(
        views.iter().any(|v| v.name == "user_emails"),
        "user_emails view not found in {:?}",
        views
    );

    // Indexes
    let indexes = driver.list_indexes(&conn_id, "main").await.unwrap();
    assert!(
        indexes.iter().any(|i| i.name == "idx_posts_user"),
        "idx_posts_user not found in {:?}",
        indexes
    );

    // Foreign keys
    let fks = driver
        .list_foreign_keys(&conn_id, "main", "posts")
        .await
        .unwrap();
    assert!(
        !fks.is_empty(),
        "expected foreign key on posts"
    );

    // Triggers
    let triggers = driver
        .list_triggers(&conn_id, "main", "posts")
        .await
        .unwrap();
    assert!(
        triggers.iter().any(|t| t.name == "trg_posts"),
        "trg_posts not found in {:?}",
        triggers
    );

    // Check constraints
    let checks = driver
        .list_check_constraints(&conn_id, "main", "users")
        .await
        .unwrap();
    let _ = checks; // just verify no error

    // Unique constraints
    let uniques = driver
        .list_unique_constraints(&conn_id, "main", "users")
        .await
        .unwrap();
    let _ = uniques;

    // These should return empty for SQLite
    let dbs = driver.list_databases(&conn_id).await.unwrap();
    assert!(dbs.is_empty());

    let schemas = driver.list_schemas(&conn_id).await.unwrap();
    assert!(schemas.is_empty());

    let mvs = driver
        .list_materialized_views(&conn_id, "main")
        .await
        .unwrap();
    assert!(mvs.is_empty());

    let funcs = driver.list_functions(&conn_id, "main").await.unwrap();
    assert!(funcs.is_empty());

    let seqs = driver.list_sequences(&conn_id, "main").await.unwrap();
    assert!(seqs.is_empty());

    let fts = driver.list_foreign_tables(&conn_id, "main").await.unwrap();
    assert!(fts.is_empty());

    let pinfo = driver
        .get_partition_info(&conn_id, "main", "users")
        .await
        .unwrap();
    assert!(pinfo.is_none());

    let _ = driver.disconnect(&conn_id).await;
}

// ── Test 9: ddl_and_erd ──

#[tokio::test]
async fn ddl_and_erd() {
    let (driver, conn_id, _tmp) = setup().await;

    driver
        .execute_batch(
            &conn_id,
            "CREATE TABLE authors (id INTEGER PRIMARY KEY, name TEXT NOT NULL);
             CREATE TABLE books (
                 id INTEGER PRIMARY KEY,
                 author_id INTEGER REFERENCES authors(id),
                 title TEXT NOT NULL
             )",
        )
        .await
        .expect("create tables failed");

    // DDL
    let ddl = driver
        .get_create_table_sql(&conn_id, "main", "books")
        .await
        .expect("get_create_table_sql failed");
    assert!(
        ddl.to_lowercase().contains("create table"),
        "DDL should contain CREATE TABLE: {}",
        ddl
    );
    assert!(
        ddl.contains("books"),
        "DDL should mention table name: {}",
        ddl
    );

    // ERD
    let erd = driver
        .get_erd_data(&conn_id, "main")
        .await
        .expect("get_erd_data failed");
    assert!(erd.tables.iter().any(|t| t.name == "authors"));
    assert!(erd.tables.iter().any(|t| t.name == "books"));
    assert!(
        erd.columns.contains_key("books"),
        "ERD should have columns for books"
    );

    let _ = driver.disconnect(&conn_id).await;
}

// ── Test 10: completion_bundle ──

#[tokio::test]
async fn completion_bundle() {
    let (driver, conn_id, _tmp) = setup().await;

    driver
        .execute_batch(
            &conn_id,
            "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT);
             CREATE VIEW item_names AS SELECT name FROM items",
        )
        .await
        .expect("create objects failed");

    let bundle = driver
        .get_completion_bundle(&conn_id, "main")
        .await
        .expect("get_completion_bundle failed");

    assert!(
        bundle.tables.iter().any(|t| t.name == "items"),
        "completion bundle should contain items: {:?}",
        bundle.tables
    );
    assert!(
        bundle.tables.iter().any(|t| t.name == "item_names"),
        "completion bundle should contain item_names view: {:?}",
        bundle.tables
    );

    // Table columns for completion
    let cols = driver
        .get_table_columns_for_completion(&conn_id, "main", "items")
        .await
        .expect("get_table_columns_for_completion failed");
    assert!(cols.iter().any(|c| c.name == "id"));
    assert!(cols.iter().any(|c| c.name == "name"));

    // Schema completion data
    let schema_data = driver
        .get_schema_completion_data(&conn_id, "main")
        .await
        .expect("get_schema_completion_data failed");
    assert!(
        schema_data.contains_key("items"),
        "schema completion should contain items"
    );

    let _ = driver.disconnect(&conn_id).await;
}

// ── Test 11: error_paths ──

#[tokio::test]
async fn error_paths() {
    let (driver, conn_id, _tmp) = setup().await;

    // Invalid SQL
    let result = driver
        .execute(&conn_id, "SELEC invalid syntax here!!!")
        .await;
    assert!(result.is_err());
    match result.unwrap_err() {
        SakiError::QueryFailed(msg) => {
            assert!(!msg.is_empty(), "error message should be non-empty");
        }
        other => panic!("expected QueryFailed, got {:?}", other),
    }

    let _ = driver.disconnect(&conn_id).await;

    // Nonexistent file should fail to connect
    let driver2 = SqliteDriver::new();
    let config = make_config("/nonexistent/path/that/does/not/exist/db.sqlite");
    let result = driver2.connect(&config).await;
    assert!(result.is_err(), "connecting to nonexistent path should fail");
    match result.unwrap_err() {
        SakiError::ConnectionFailed(_) => {}
        other => panic!("expected ConnectionFailed, got {:?}", other),
    }
}

// ── Test 12: export_roundtrip ──

#[tokio::test(flavor = "multi_thread")]
async fn export_roundtrip() {
    let (driver, conn_id, _tmp) = setup().await;

    driver
        .execute_batch(
            &conn_id,
            "CREATE TABLE export_data (id INTEGER, val TEXT);
             INSERT INTO export_data VALUES (1, 'alpha');
             INSERT INTO export_data VALUES (2, 'beta');
             INSERT INTO export_data VALUES (3, 'gamma')",
        )
        .await
        .expect("setup export data failed");

    let cancelled = AtomicBool::new(false);
    let collected_rows = Arc::new(std::sync::Mutex::new(Vec::new()));
    let rows_clone = Arc::clone(&collected_rows);

    let on_batch: Box<ExportBatchFn> = Box::new(move |cols, cells, _total| {
        let num_cols = cols.len();
        let num_rows = cells.len() / num_cols;
        let mut rows = rows_clone.lock().unwrap();
        for r in 0..num_rows {
            let mut row = Vec::new();
            for c in 0..num_cols {
                row.push(cells[r * num_cols + c].clone());
            }
            rows.push(row);
        }
        Ok(())
    });

    let total = driver
        .export_stream(
            &conn_id,
            "SELECT id, val FROM export_data ORDER BY id",
            2, // small batch size
            &cancelled,
            &*on_batch,
        )
        .await
        .expect("export_stream failed");

    assert_eq!(total, 3, "should export 3 rows");

    let rows = collected_rows.lock().unwrap();
    assert_eq!(rows.len(), 3);

    // Verify first row
    match &rows[0][0] {
        CellValue::Int(n) => assert_eq!(*n, 1),
        other => panic!("expected Int(1), got {:?}", other),
    }
    match &rows[0][1] {
        CellValue::Text(s) => assert_eq!(&**s, "alpha"),
        other => panic!("expected Text(alpha), got {:?}", other),
    }

    let _ = driver.disconnect(&conn_id).await;
}

// ── Test 13: restore_roundtrip ──

#[tokio::test(flavor = "multi_thread")]
async fn restore_roundtrip() {
    let (driver, conn_id, _tmp) = setup().await;

    // Write a SQL file to restore
    let mut sql_file = NamedTempFile::new().expect("create sql temp file");
    writeln!(
        sql_file,
        "CREATE TABLE restored (id INTEGER PRIMARY KEY, val TEXT);
         INSERT INTO restored VALUES (1, 'one');
         INSERT INTO restored VALUES (2, 'two');
         INSERT INTO restored VALUES (3, 'three');"
    )
    .expect("write sql file");

    let sql_path = sql_file.path().to_str().unwrap().to_string();
    let cancelled = AtomicBool::new(false);
    let options = RestoreOptions {
        schema: None,
        continue_on_error: false,
    };

    let progress = driver
        .restore(
            &conn_id,
            &sql_path,
            &options,
            &cancelled,
            Box::new(|_p| {}),
        )
        .await
        .expect("restore failed");

    assert!(
        progress.statements_executed > 0,
        "should have executed statements"
    );
    assert!(progress.error.is_none(), "should have no error");

    // Verify data was restored
    let result = driver
        .execute(&conn_id, "SELECT COUNT(*) FROM restored")
        .await
        .expect("count query failed");

    match &result.cells[0] {
        CellValue::Int(n) => assert_eq!(*n, 3, "expected 3 restored rows"),
        other => panic!("expected Int(3), got {:?}", other),
    }

    let _ = driver.disconnect(&conn_id).await;
}

// ── Test 14: data_type_roundtrip ──

#[tokio::test]
async fn data_type_roundtrip() {
    let (driver, conn_id, _tmp) = setup().await;

    driver
        .execute_batch(
            &conn_id,
            "CREATE TABLE types_test (
                i INTEGER,
                r REAL,
                t TEXT,
                b BLOB
            );
            INSERT INTO types_test VALUES (42, 3.14, 'hello', X'DEADBEEF')",
        )
        .await
        .expect("insert types failed");

    let result = driver
        .execute(&conn_id, "SELECT i, r, t, b FROM types_test")
        .await
        .expect("select types failed");

    assert_eq!(result.row_count, 1);
    assert_eq!(result.columns.len(), 4);

    // Integer
    match &result.cells[0] {
        CellValue::Int(n) => assert_eq!(*n, 42),
        other => panic!("expected Int(42), got {:?}", other),
    }
    // Real
    match &result.cells[1] {
        CellValue::Float(f) => assert!((f - 3.14).abs() < 0.001),
        other => panic!("expected Float(3.14), got {:?}", other),
    }
    // Text
    match &result.cells[2] {
        CellValue::Text(s) => assert_eq!(&**s, "hello"),
        other => panic!("expected Text(hello), got {:?}", other),
    }
    // Blob
    match &result.cells[3] {
        CellValue::Bytes(b) => assert_eq!(&**b, &[0xDE, 0xAD, 0xBE, 0xEF]),
        other => panic!("expected Bytes(DEADBEEF), got {:?}", other),
    }

    let _ = driver.disconnect(&conn_id).await;
}

// ── Test 15: concurrent_read_wal_mode ──

#[tokio::test]
async fn concurrent_read_wal_mode() {
    let (driver, conn_id, _tmp) = setup().await;
    let driver = Arc::new(driver);

    // Create table and insert initial data
    driver
        .execute_batch(
            &conn_id,
            "CREATE TABLE wal_test (id INTEGER PRIMARY KEY, val TEXT);
             INSERT INTO wal_test VALUES (1, 'initial')",
        )
        .await
        .expect("setup failed");

    // Spawn multiple concurrent readers
    let mut handles = Vec::new();
    for i in 0..5 {
        let d = Arc::clone(&driver);
        let cid = conn_id;
        handles.push(tokio::spawn(async move {
            let result = d
                .execute(&cid, "SELECT COUNT(*) FROM wal_test")
                .await
                .unwrap_or_else(|e| panic!("reader {i} failed: {:?}", e));
            match &result.cells[0] {
                CellValue::Int(n) => assert!(*n >= 1, "reader {i} should see at least 1 row"),
                other => panic!("reader {i}: expected Int, got {:?}", other),
            }
        }));
    }

    // Write while readers are running
    driver
        .execute_batch(
            &conn_id,
            "INSERT INTO wal_test VALUES (2, 'concurrent')",
        )
        .await
        .expect("concurrent write failed");

    // Wait for all readers
    for handle in handles {
        handle.await.expect("reader task panicked");
    }

    // Verify final state
    let result = driver
        .execute(&conn_id, "SELECT COUNT(*) FROM wal_test")
        .await
        .expect("final count failed");
    match &result.cells[0] {
        CellValue::Int(n) => assert_eq!(*n, 2),
        other => panic!("expected Int(2), got {:?}", other),
    }

    let _ = driver.disconnect(&conn_id).await;
}

// ── Test 16: large_file_restore ──

#[tokio::test(flavor = "multi_thread")]
async fn large_file_restore() {
    let (driver, conn_id, _tmp) = setup().await;

    // Generate a SQL file with 10K insert statements
    let mut sql_file = NamedTempFile::new().expect("create sql temp file");
    writeln!(sql_file, "CREATE TABLE big_table (id INTEGER PRIMARY KEY, val TEXT);")
        .expect("write create");

    for i in 1..=10_000 {
        writeln!(
            sql_file,
            "INSERT INTO big_table VALUES ({i}, 'row_{i}');"
        )
        .expect("write insert");
    }

    let sql_path = sql_file.path().to_str().unwrap().to_string();
    let cancelled = AtomicBool::new(false);
    let options = RestoreOptions {
        schema: None,
        continue_on_error: false,
    };

    let progress_reports = Arc::new(std::sync::Mutex::new(Vec::new()));
    let reports_clone = Arc::clone(&progress_reports);

    let progress = driver
        .restore(
            &conn_id,
            &sql_path,
            &options,
            &cancelled,
            Box::new(move |p| {
                reports_clone.lock().unwrap().push(p.statements_executed);
            }),
        )
        .await
        .expect("large restore failed");

    assert!(
        progress.statements_executed >= 10_000,
        "expected at least 10000 statements, got {}",
        progress.statements_executed
    );
    assert!(progress.error.is_none(), "should have no error");

    // Verify progress was reported
    let reports = progress_reports.lock().unwrap();
    assert!(
        !reports.is_empty(),
        "should have received progress reports"
    );

    // Verify data
    let result = driver
        .execute(&conn_id, "SELECT COUNT(*) FROM big_table")
        .await
        .expect("count query failed");

    match &result.cells[0] {
        CellValue::Int(n) => assert_eq!(*n, 10_000),
        other => panic!("expected Int(10000), got {:?}", other),
    }

    let _ = driver.disconnect(&conn_id).await;
}
