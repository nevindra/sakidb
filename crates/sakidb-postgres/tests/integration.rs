#![cfg(feature = "integration")]

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use sakidb_core::types::*;
use sakidb_core::{Driver, Exporter, Introspector, SakiError, SqlDriver};
use sakidb_postgres::PostgresDriver;

/// Parse TEST_DATABASE_URL into a ConnectionConfig.
///
/// Expected format: postgres://user:pass@host:port/dbname
/// Falls back to reasonable defaults if parsing fails.
fn make_config() -> ConnectionConfig {
    let url = std::env::var("TEST_DATABASE_URL").expect(
        "TEST_DATABASE_URL must be set (e.g. postgres://postgres:postgres@localhost:5432/sakidb_test)",
    );

    // Simple URL parsing: postgres://user:pass@host:port/dbname
    let without_scheme = url
        .strip_prefix("postgres://")
        .or_else(|| url.strip_prefix("postgresql://"))
        .unwrap_or(&url);

    let (userinfo, hostinfo) = without_scheme
        .split_once('@')
        .unwrap_or(("postgres:", without_scheme));
    let (username, password) = userinfo.split_once(':').unwrap_or((userinfo, ""));
    let (hostport, database) = hostinfo.split_once('/').unwrap_or((hostinfo, "sakidb_test"));
    let (host, port_str) = hostport.split_once(':').unwrap_or((hostport, "5432"));
    let port: u16 = port_str.parse().unwrap_or(5432);

    ConnectionConfig {
        engine: EngineType::Postgres,
        host: host.to_string(),
        port,
        database: database.to_string(),
        username: username.to_string(),
        password: password.to_string(),
        ssl_mode: SslMode::Disable,
        options: HashMap::new(),
    }
}

/// Create a unique schema name to isolate each test.
fn unique_schema() -> String {
    let id = uuid::Uuid::new_v4().to_string().replace('-', "");
    format!("integ_{}", &id[..12])
}

/// Connect a fresh driver and return (driver, connection_id).
async fn setup() -> (PostgresDriver, ConnectionId) {
    let driver = PostgresDriver::new();
    let config = make_config();
    let conn_id = driver.connect(&config).await.expect("connect failed");
    (driver, conn_id)
}

/// Setup with an isolated schema. Returns (driver, conn_id, schema_name).
async fn setup_with_schema() -> (PostgresDriver, ConnectionId, String) {
    let (driver, conn_id) = setup().await;
    let schema = unique_schema();
    driver
        .execute_batch(
            &conn_id,
            &format!("CREATE SCHEMA {schema}; SET search_path TO {schema}"),
        )
        .await
        .expect("create schema failed");
    (driver, conn_id, schema)
}

/// Tear down: drop the schema and disconnect.
async fn teardown(driver: &PostgresDriver, conn_id: &ConnectionId, schema: &str) {
    let _ = driver
        .execute_batch(conn_id, &format!("DROP SCHEMA IF EXISTS {schema} CASCADE"))
        .await;
    let _ = driver.disconnect(conn_id).await;
}

// ── Test 1: connect_and_disconnect ──

#[tokio::test]
async fn connect_and_disconnect() {
    let driver = PostgresDriver::new();
    let config = make_config();

    let conn_id = driver.connect(&config).await.expect("connect failed");
    driver
        .disconnect(&conn_id)
        .await
        .expect("disconnect failed");
}

// ── Test 2: execute_select ──

#[tokio::test]
async fn execute_select() {
    let (driver, conn_id) = setup().await;

    let result = driver
        .execute(&conn_id, "SELECT 1 AS num, 'hello' AS msg")
        .await
        .expect("execute failed");

    assert_eq!(result.columns.len(), 2);
    assert_eq!(result.columns[0].name, "num");
    assert_eq!(result.columns[1].name, "msg");
    assert_eq!(result.row_count, 1);

    // cells are flat: row0_col0, row0_col1
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
    let (driver, conn_id) = setup().await;

    let result = driver
        .execute_multi(&conn_id, "SELECT 1 AS a; SELECT 2 AS b")
        .await
        .expect("execute_multi failed");

    assert_eq!(result.results.len(), 2);
    assert_eq!(result.results[0].columns[0].name, "a");
    assert_eq!(result.results[1].columns[0].name, "b");

    match &result.results[0].cells[0] {
        CellValue::Int(n) => assert_eq!(*n, 1),
        other => panic!("expected Int(1), got {:?}", other),
    }
    match &result.results[1].cells[0] {
        CellValue::Int(n) => assert_eq!(*n, 2),
        other => panic!("expected Int(2), got {:?}", other),
    }

    let _ = driver.disconnect(&conn_id).await;
}

// ── Test 4: execute_columnar ──

#[tokio::test]
async fn execute_columnar() {
    let (driver, conn_id) = setup().await;

    let result = driver
        .execute_multi_columnar(&conn_id, "SELECT 42 AS num, 'text' AS val, true AS flag")
        .await
        .expect("execute_multi_columnar failed");

    assert_eq!(result.results.len(), 1);
    let cr = &result.results[0];
    assert_eq!(cr.columns.len(), 3);
    assert_eq!(cr.row_count, 1);

    // Verify column types
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
    match &cr.column_data[2] {
        ColumnStorage::Bool { values, .. } => assert_eq!(values[0], 1),
        other => panic!("expected Bool column, got {:?}", other),
    }

    let _ = driver.disconnect(&conn_id).await;
}

// ── Test 5: execute_paged_and_batch ──

#[tokio::test]
async fn execute_paged_and_batch() {
    let (driver, conn_id, schema) = setup_with_schema().await;

    driver
        .execute_batch(
            &conn_id,
            &format!(
                "CREATE TABLE {schema}.paged_test (id INT); {}",
                (1..=100)
                    .map(|i| format!("INSERT INTO {schema}.paged_test VALUES ({i})"))
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
        )
        .await
        .expect("batch insert failed");

    // Page 0: rows 1-25 (0-based paging)
    let page0 = driver
        .execute_paged(
            &conn_id,
            &format!("SELECT id FROM {schema}.paged_test ORDER BY id"),
            0,
            25,
        )
        .await
        .expect("execute_paged failed");

    assert_eq!(page0.row_count, 25);
    assert_eq!(page0.page, 0);
    assert_eq!(page0.page_size, 25);

    // First row should be 1
    match &page0.cells[0] {
        CellValue::Int(n) => assert_eq!(*n, 1),
        other => panic!("expected Int(1), got {:?}", other),
    }

    // Page 1: rows 26-50 (0-based: offset = 1*25 = 25)
    let page1 = driver
        .execute_paged(
            &conn_id,
            &format!("SELECT id FROM {schema}.paged_test ORDER BY id"),
            1,
            25,
        )
        .await
        .expect("execute_paged page 1 failed");

    assert_eq!(page1.row_count, 25);
    match &page1.cells[0] {
        CellValue::Int(n) => assert_eq!(*n, 26),
        other => panic!("expected Int(26), got {:?}", other),
    }

    teardown(&driver, &conn_id, &schema).await;
}

// ── Test 6: list_tables_after_create ──

#[tokio::test]
async fn list_tables_after_create() {
    let (driver, conn_id, schema) = setup_with_schema().await;

    driver
        .execute_batch(
            &conn_id,
            &format!("CREATE TABLE {schema}.my_test_table (id SERIAL PRIMARY KEY, name TEXT)"),
        )
        .await
        .expect("create table failed");

    let tables = driver
        .list_tables(&conn_id, &schema)
        .await
        .expect("list_tables failed");

    let found = tables.iter().any(|t| t.name == "my_test_table");
    assert!(found, "my_test_table not found in {:?}", tables);

    teardown(&driver, &conn_id, &schema).await;
}

// ── Test 7: list_columns_types ──

#[tokio::test]
async fn list_columns_types() {
    let (driver, conn_id, schema) = setup_with_schema().await;

    driver
        .execute_batch(
            &conn_id,
            &format!(
                "CREATE TABLE {schema}.typed_cols (
                    id INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    active BOOLEAN DEFAULT true,
                    score DOUBLE PRECISION
                )"
            ),
        )
        .await
        .expect("create table failed");

    let columns = driver
        .list_columns(&conn_id, &schema, "typed_cols")
        .await
        .expect("list_columns failed");

    assert_eq!(columns.len(), 4);

    let id_col = columns.iter().find(|c| c.name == "id").unwrap();
    assert!(id_col.is_primary_key);
    assert!(!id_col.is_nullable);

    let name_col = columns.iter().find(|c| c.name == "name").unwrap();
    assert!(!name_col.is_nullable);

    let active_col = columns.iter().find(|c| c.name == "active").unwrap();
    assert!(active_col.is_nullable);
    assert!(active_col.default_value.is_some());

    let score_col = columns.iter().find(|c| c.name == "score").unwrap();
    assert!(score_col.is_nullable);

    teardown(&driver, &conn_id, &schema).await;
}

// ── Test 8: introspect_full_schema ──

#[tokio::test]
async fn introspect_full_schema() {
    let (driver, conn_id, schema) = setup_with_schema().await;

    driver
        .execute_batch(
            &conn_id,
            &format!(
                "CREATE TABLE {schema}.users (id SERIAL PRIMARY KEY, email TEXT UNIQUE NOT NULL);
                 CREATE TABLE {schema}.posts (
                     id SERIAL PRIMARY KEY,
                     user_id INT REFERENCES {schema}.users(id),
                     title TEXT NOT NULL
                 );
                 CREATE VIEW {schema}.user_emails AS SELECT id, email FROM {schema}.users;
                 CREATE INDEX idx_posts_user ON {schema}.posts (user_id)"
            ),
        )
        .await
        .expect("setup objects failed");

    // Tables
    let tables = driver.list_tables(&conn_id, &schema).await.unwrap();
    assert!(tables.iter().any(|t| t.name == "users"));
    assert!(tables.iter().any(|t| t.name == "posts"));

    // Views
    let views = driver.list_views(&conn_id, &schema).await.unwrap();
    assert!(views.iter().any(|v| v.name == "user_emails"));

    // Indexes
    let indexes = driver.list_indexes(&conn_id, &schema).await.unwrap();
    assert!(indexes.iter().any(|i| i.name == "idx_posts_user"));

    // Schemas (should include our test schema)
    let schemas = driver.list_schemas(&conn_id).await.unwrap();
    assert!(schemas.iter().any(|s| s.name == schema));

    // Foreign keys on posts
    let fks = driver
        .list_foreign_keys(&conn_id, &schema, "posts")
        .await
        .unwrap();
    assert!(!fks.is_empty());
    assert_eq!(fks[0].foreign_table_name, "users");

    // Triggers (empty is fine for this test)
    let triggers = driver
        .list_triggers(&conn_id, &schema, "users")
        .await
        .unwrap();
    let _ = triggers; // just verify it doesn't error

    // Check constraints
    let checks = driver
        .list_check_constraints(&conn_id, &schema, "users")
        .await
        .unwrap();
    let _ = checks;

    // Unique constraints (users has UNIQUE on email)
    let uniques = driver
        .list_unique_constraints(&conn_id, &schema, "users")
        .await
        .unwrap();
    assert!(
        uniques.iter().any(|u| u.columns.contains(&"email".to_string())),
        "expected unique constraint on email, got {:?}",
        uniques
    );

    // Materialized views (empty)
    let mvs = driver
        .list_materialized_views(&conn_id, &schema)
        .await
        .unwrap();
    let _ = mvs;

    // Functions (empty for our test schema)
    let funcs = driver.list_functions(&conn_id, &schema).await.unwrap();
    let _ = funcs;

    // Sequences (serial columns create sequences)
    let seqs = driver.list_sequences(&conn_id, &schema).await.unwrap();
    assert!(!seqs.is_empty(), "expected sequences from SERIAL columns");

    // Foreign tables (empty)
    let fts = driver.list_foreign_tables(&conn_id, &schema).await.unwrap();
    let _ = fts;

    // Partition info (no partitions)
    let pinfo = driver
        .get_partition_info(&conn_id, &schema, "users")
        .await
        .unwrap();
    assert!(pinfo.is_none());

    teardown(&driver, &conn_id, &schema).await;
}

// ── Test 9: ddl_and_erd ──

#[tokio::test]
async fn ddl_and_erd() {
    let (driver, conn_id, schema) = setup_with_schema().await;

    driver
        .execute_batch(
            &conn_id,
            &format!(
                "CREATE TABLE {schema}.authors (id SERIAL PRIMARY KEY, name TEXT NOT NULL);
                 CREATE TABLE {schema}.books (
                     id SERIAL PRIMARY KEY,
                     author_id INT REFERENCES {schema}.authors(id),
                     title TEXT NOT NULL
                 )"
            ),
        )
        .await
        .expect("create tables failed");

    // DDL
    let ddl = driver
        .get_create_table_sql(&conn_id, &schema, "books")
        .await
        .expect("get_create_table_sql failed");
    assert!(ddl.contains("books"), "DDL should mention table name");
    assert!(
        ddl.to_lowercase().contains("create table"),
        "DDL should contain CREATE TABLE"
    );

    // ERD
    let erd = driver
        .get_erd_data(&conn_id, &schema)
        .await
        .expect("get_erd_data failed");
    assert!(erd.tables.iter().any(|t| t.name == "authors"));
    assert!(erd.tables.iter().any(|t| t.name == "books"));
    assert!(
        erd.columns.contains_key("books"),
        "ERD should have columns for books"
    );
    assert!(
        erd.foreign_keys.contains_key("books"),
        "ERD should have FK data for books"
    );

    teardown(&driver, &conn_id, &schema).await;
}

// ── Test 10: completion_bundle ──

#[tokio::test]
async fn completion_bundle() {
    let (driver, conn_id, schema) = setup_with_schema().await;

    driver
        .execute_batch(
            &conn_id,
            &format!(
                "CREATE TABLE {schema}.items (id INT PRIMARY KEY, name TEXT);
                 CREATE VIEW {schema}.item_names AS SELECT name FROM {schema}.items"
            ),
        )
        .await
        .expect("create objects failed");

    let bundle = driver
        .get_completion_bundle(&conn_id, &schema)
        .await
        .expect("get_completion_bundle failed");

    assert!(
        bundle.tables.iter().any(|t| t.name == "items"),
        "completion bundle should contain items table"
    );
    assert!(
        bundle.tables.iter().any(|t| t.name == "item_names" && t.kind == "view"),
        "completion bundle should contain item_names view"
    );

    // Also test table columns for completion
    let cols = driver
        .get_table_columns_for_completion(&conn_id, &schema, "items")
        .await
        .expect("get_table_columns_for_completion failed");
    assert!(cols.iter().any(|c| c.name == "id" && c.is_primary_key));
    assert!(cols.iter().any(|c| c.name == "name"));

    // Schema completion data
    let schema_data = driver
        .get_schema_completion_data(&conn_id, &schema)
        .await
        .expect("get_schema_completion_data failed");
    assert!(
        schema_data.contains_key("items"),
        "schema completion should contain items"
    );

    teardown(&driver, &conn_id, &schema).await;
}

// ── Test 11: cancel_long_query ──

#[tokio::test]
async fn cancel_long_query() {
    let (driver, conn_id) = setup().await;
    let driver = Arc::new(driver);

    let driver_clone = Arc::clone(&driver);
    let conn_id_clone = conn_id;

    // Spawn a long query
    let query_handle = tokio::spawn(async move {
        driver_clone
            .execute(&conn_id_clone, "SELECT pg_sleep(60)")
            .await
    });

    // Give the query a moment to start
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Cancel
    driver
        .cancel_query(&conn_id)
        .await
        .expect("cancel_query failed");

    // The query should fail with Cancelled or QueryFailed
    let result = query_handle.await.expect("task panicked");
    assert!(
        result.is_err(),
        "expected error after cancel, got {:?}",
        result
    );
    match result.unwrap_err() {
        SakiError::Cancelled | SakiError::QueryFailed(_) => {}
        other => panic!("expected Cancelled or QueryFailed, got {:?}", other),
    }

    let _ = driver.disconnect(&conn_id).await;
}

// ── Test 12: error_paths ──

#[tokio::test]
async fn error_paths() {
    let (driver, conn_id) = setup().await;

    // Invalid SQL should give QueryFailed
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

    // Non-existent table
    let result = driver
        .execute(&conn_id, "SELECT * FROM nonexistent_table_xyz_abc_123")
        .await;
    assert!(result.is_err());
    match result.unwrap_err() {
        SakiError::QueryFailed(_) => {}
        other => panic!("expected QueryFailed, got {:?}", other),
    }

    let _ = driver.disconnect(&conn_id).await;
}

// ── Test 13: export_sql_and_cancel ──

#[tokio::test]
async fn export_sql_and_cancel() {
    let (driver, conn_id, schema) = setup_with_schema().await;

    driver
        .execute_batch(
            &conn_id,
            &format!(
                "CREATE TABLE {schema}.export_test (id INT, val TEXT);
                 {}",
                (1..=50)
                    .map(|i| format!(
                        "INSERT INTO {schema}.export_test VALUES ({i}, 'row_{i}')"
                    ))
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
        )
        .await
        .expect("setup export data failed");

    // Export without cancellation
    let cancelled = AtomicBool::new(false);

    let on_batch: Box<ExportBatchFn> = Box::new(move |_cols, _cells, _total| {
        Ok(())
    });

    let rows = driver
        .export_stream(
            &conn_id,
            &format!("SELECT * FROM {schema}.export_test ORDER BY id"),
            10,
            &cancelled,
            &*on_batch,
        )
        .await
        .expect("export_stream failed");

    assert_eq!(rows, 50, "expected 50 exported rows");

    teardown(&driver, &conn_id, &schema).await;
}

// ── Test 14: data_type_roundtrip ──

#[tokio::test]
async fn data_type_roundtrip() {
    let (driver, conn_id, schema) = setup_with_schema().await;

    driver
        .execute_batch(
            &conn_id,
            &format!(
                "CREATE TABLE {schema}.types_test (
                    i INTEGER,
                    t TEXT,
                    b BOOLEAN,
                    f DOUBLE PRECISION,
                    j JSONB,
                    ts TIMESTAMPTZ
                );
                INSERT INTO {schema}.types_test VALUES (
                    42,
                    'hello world',
                    true,
                    3.14,
                    '{{\"key\": \"value\"}}',
                    '2024-01-15T10:30:00Z'
                )"
            ),
        )
        .await
        .expect("insert types failed");

    let result = driver
        .execute(
            &conn_id,
            &format!("SELECT i, t, b, f, j, ts FROM {schema}.types_test"),
        )
        .await
        .expect("select types failed");

    assert_eq!(result.row_count, 1);
    assert_eq!(result.columns.len(), 6);

    // Int
    match &result.cells[0] {
        CellValue::Int(n) => assert_eq!(*n, 42),
        other => panic!("expected Int(42), got {:?}", other),
    }
    // Text
    match &result.cells[1] {
        CellValue::Text(s) => assert_eq!(&**s, "hello world"),
        other => panic!("expected Text, got {:?}", other),
    }
    // Bool
    match &result.cells[2] {
        CellValue::Bool(b) => assert!(*b),
        other => panic!("expected Bool(true), got {:?}", other),
    }
    // Float
    match &result.cells[3] {
        CellValue::Float(f) => assert!((f - 3.14).abs() < 0.001),
        other => panic!("expected Float(3.14), got {:?}", other),
    }
    // Json
    match &result.cells[4] {
        CellValue::Json(s) => {
            assert!(s.contains("key"), "json should contain 'key': {}", s);
        }
        other => panic!("expected Json, got {:?}", other),
    }
    // Timestamp
    match &result.cells[5] {
        CellValue::Timestamp(s) => {
            assert!(s.contains("2024"), "timestamp should contain year: {}", s);
        }
        other => panic!("expected Timestamp, got {:?}", other),
    }

    teardown(&driver, &conn_id, &schema).await;
}
