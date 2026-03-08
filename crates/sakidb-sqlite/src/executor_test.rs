use rusqlite::types::ValueRef;
use sakidb_core::sql::split_sql_statements;
use sakidb_core::types::CellValue;

use crate::executor::sqlite_value_to_cell;

#[test]
fn test_split_simple() {
    let stmts = split_sql_statements("SELECT 1; SELECT 2;");
    assert_eq!(stmts, vec!["SELECT 1", "SELECT 2"]);
}

#[test]
fn test_split_string_literal() {
    let stmts = split_sql_statements("SELECT 'hello;world'; SELECT 2");
    assert_eq!(stmts, vec!["SELECT 'hello;world'", "SELECT 2"]);
}

#[test]
fn test_split_line_comment() {
    let stmts = split_sql_statements("SELECT 1; -- comment;\nSELECT 2");
    assert_eq!(stmts, vec!["SELECT 1", "-- comment;\nSELECT 2"]);
}

#[test]
fn test_split_block_comment() {
    let stmts = split_sql_statements("SELECT /* ; */ 1; SELECT 2");
    assert_eq!(stmts, vec!["SELECT /* ; */ 1", "SELECT 2"]);
}

#[test]
fn test_split_empty() {
    let stmts = split_sql_statements("   ;  ;  ");
    assert!(stmts.is_empty());
}

#[test]
fn test_sqlite_value_to_cell() {
    assert!(matches!(sqlite_value_to_cell(ValueRef::Null), CellValue::Null));
    assert!(matches!(sqlite_value_to_cell(ValueRef::Integer(42)), CellValue::Int(42)));
    assert!(matches!(sqlite_value_to_cell(ValueRef::Real(3.14)), CellValue::Float(f) if (f - 3.14).abs() < f64::EPSILON));
}

// ── Additional sqlite_value_to_cell tests ──

#[test]
fn test_sqlite_value_to_cell_text() {
    let text = b"hello world";
    let cell = sqlite_value_to_cell(ValueRef::Text(text));
    match cell {
        CellValue::Text(s) => assert_eq!(&*s, "hello world"),
        other => panic!("expected Text, got {:?}", other),
    }
}

#[test]
fn test_sqlite_value_to_cell_blob() {
    let blob = &[0x00, 0x01, 0x02, 0xFF];
    let cell = sqlite_value_to_cell(ValueRef::Blob(blob));
    match cell {
        CellValue::Bytes(b) => assert_eq!(&*b, blob),
        other => panic!("expected Bytes, got {:?}", other),
    }
}

#[test]
fn test_sqlite_value_to_cell_empty_text() {
    let cell = sqlite_value_to_cell(ValueRef::Text(b""));
    match cell {
        CellValue::Text(s) => assert_eq!(&*s, ""),
        other => panic!("expected Text, got {:?}", other),
    }
}

#[test]
fn test_sqlite_value_to_cell_empty_blob() {
    let cell = sqlite_value_to_cell(ValueRef::Blob(&[]));
    match cell {
        CellValue::Bytes(b) => assert!(b.is_empty()),
        other => panic!("expected Bytes, got {:?}", other),
    }
}

#[test]
fn test_sqlite_value_to_cell_negative_integer() {
    let cell = sqlite_value_to_cell(ValueRef::Integer(-1));
    assert!(matches!(cell, CellValue::Int(-1)));
}

#[test]
fn test_sqlite_value_to_cell_zero_integer() {
    let cell = sqlite_value_to_cell(ValueRef::Integer(0));
    assert!(matches!(cell, CellValue::Int(0)));
}

#[test]
fn test_sqlite_value_to_cell_large_integer() {
    let cell = sqlite_value_to_cell(ValueRef::Integer(i64::MAX));
    assert!(matches!(cell, CellValue::Int(v) if v == i64::MAX));
}

#[test]
fn test_sqlite_value_to_cell_nan_like_float() {
    let cell = sqlite_value_to_cell(ValueRef::Real(f64::INFINITY));
    assert!(matches!(cell, CellValue::Float(f) if f.is_infinite()));
}

#[test]
fn test_sqlite_value_to_cell_negative_float() {
    let cell = sqlite_value_to_cell(ValueRef::Real(-2.5));
    assert!(matches!(cell, CellValue::Float(f) if (f - (-2.5)).abs() < f64::EPSILON));
}

#[test]
fn test_sqlite_value_to_cell_utf8_text() {
    let text = "日本語テスト".as_bytes();
    let cell = sqlite_value_to_cell(ValueRef::Text(text));
    match cell {
        CellValue::Text(s) => assert_eq!(&*s, "日本語テスト"),
        other => panic!("expected Text, got {:?}", other),
    }
}

// ── Additional split_sql edge cases ──

#[test]
fn test_split_no_trailing_semicolon() {
    let stmts = split_sql_statements("SELECT 1; SELECT 2");
    assert_eq!(stmts, vec!["SELECT 1", "SELECT 2"]);
}

#[test]
fn test_split_escaped_quote() {
    let stmts = split_sql_statements("SELECT 'it''s'; SELECT 2");
    assert_eq!(stmts, vec!["SELECT 'it''s'", "SELECT 2"]);
}

#[test]
fn test_split_single() {
    let stmts = split_sql_statements("SELECT 1");
    assert_eq!(stmts, vec!["SELECT 1"]);
}

#[test]
fn test_split_nested_block_comments() {
    let stmts = split_sql_statements("SELECT /* outer /* inner */ still */ 1; SELECT 2");
    assert_eq!(stmts, vec!["SELECT /* outer /* inner */ still */ 1", "SELECT 2"]);
}

#[test]
fn test_split_multiple_semicolons() {
    let stmts = split_sql_statements("SELECT 1;;; SELECT 2");
    assert_eq!(stmts, vec!["SELECT 1", "SELECT 2"]);
}

#[test]
fn test_split_whitespace_between() {
    let stmts = split_sql_statements("  SELECT 1  ;  SELECT 2  ;  ");
    assert_eq!(stmts, vec!["SELECT 1", "SELECT 2"]);
}

#[test]
fn test_split_double_quoted_identifier() {
    let stmts = split_sql_statements("SELECT \"col;name\" FROM t; SELECT 2");
    assert_eq!(stmts.len(), 2);
    assert!(stmts[0].contains("\"col;name\""));
}

// ── In-memory SQLite execution tests ──

#[test]
fn test_execute_query_simple() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    let result = crate::executor::execute_query(&conn, "SELECT 1 AS val, 'hello' AS msg").unwrap();
    assert_eq!(result.row_count, 1);
    assert_eq!(result.columns.len(), 2);
    assert_eq!(result.columns[0].name, "val");
    assert_eq!(result.columns[1].name, "msg");
    assert!(!result.truncated);
    assert!(matches!(&result.cells[0], CellValue::Int(1)));
    match &result.cells[1] {
        CellValue::Text(s) => assert_eq!(&**s, "hello"),
        other => panic!("expected Text, got {:?}", other),
    }
}

#[test]
fn test_execute_query_empty_result() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE t (id INTEGER)").unwrap();
    let result = crate::executor::execute_query(&conn, "SELECT * FROM t").unwrap();
    assert_eq!(result.row_count, 0);
    assert!(result.cells.is_empty());
    assert!(!result.truncated);
}

#[test]
fn test_execute_query_multiple_rows() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE t (id INTEGER, name TEXT);
         INSERT INTO t VALUES (1, 'Alice');
         INSERT INTO t VALUES (2, 'Bob');
         INSERT INTO t VALUES (3, 'Charlie');"
    ).unwrap();
    let result = crate::executor::execute_query(&conn, "SELECT * FROM t ORDER BY id").unwrap();
    assert_eq!(result.row_count, 3);
    assert_eq!(result.columns.len(), 2);
    // 3 rows * 2 columns = 6 cells
    assert_eq!(result.cells.len(), 6);
}

#[test]
fn test_execute_query_with_nulls() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE t (id INTEGER, val TEXT);
         INSERT INTO t VALUES (1, NULL);"
    ).unwrap();
    let result = crate::executor::execute_query(&conn, "SELECT * FROM t").unwrap();
    assert_eq!(result.row_count, 1);
    assert!(matches!(result.cells[1], CellValue::Null));
}

#[test]
fn test_execute_multi_empty() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    let result = crate::executor::execute_multi(&conn, "").unwrap();
    assert!(result.results.is_empty());
    assert_eq!(result.total_execution_time_ms, 0);
}

#[test]
fn test_execute_multi_two_statements() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    let result = crate::executor::execute_multi(&conn, "SELECT 1; SELECT 2").unwrap();
    assert_eq!(result.results.len(), 2);
    assert!(matches!(&result.results[0].cells[0], CellValue::Int(1)));
    assert!(matches!(&result.results[1].cells[0], CellValue::Int(2)));
}

#[test]
fn test_execute_batch_creates_table() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    crate::executor::execute_batch(&conn, "CREATE TABLE t (id INTEGER); INSERT INTO t VALUES (1);").unwrap();
    let result = crate::executor::execute_query(&conn, "SELECT * FROM t").unwrap();
    assert_eq!(result.row_count, 1);
}

#[test]
fn test_execute_batch_error() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    let result = crate::executor::execute_batch(&conn, "INVALID SQL GIBBERISH");
    assert!(result.is_err());
}

#[test]
fn test_execute_paged_first_page() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE t (id INTEGER);
         INSERT INTO t VALUES (1);
         INSERT INTO t VALUES (2);
         INSERT INTO t VALUES (3);
         INSERT INTO t VALUES (4);
         INSERT INTO t VALUES (5);"
    ).unwrap();
    let result = crate::executor::execute_paged(&conn, "SELECT * FROM t ORDER BY id", 0, 2).unwrap();
    assert_eq!(result.page, 0);
    assert_eq!(result.page_size, 2);
    assert_eq!(result.row_count, 2);
    // First page should have a total rows estimate
    assert_eq!(result.total_rows_estimate, Some(5));
}

#[test]
fn test_execute_paged_second_page() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE t (id INTEGER);
         INSERT INTO t VALUES (1);
         INSERT INTO t VALUES (2);
         INSERT INTO t VALUES (3);
         INSERT INTO t VALUES (4);
         INSERT INTO t VALUES (5);"
    ).unwrap();
    let result = crate::executor::execute_paged(&conn, "SELECT * FROM t ORDER BY id", 1, 2).unwrap();
    assert_eq!(result.page, 1);
    assert_eq!(result.row_count, 2);
    // Non-first page should not have a total rows estimate
    assert_eq!(result.total_rows_estimate, None);
}

#[test]
fn test_execute_query_columnar_simple() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE t (id INTEGER, name TEXT, score REAL);
         INSERT INTO t VALUES (1, 'Alice', 95.5);
         INSERT INTO t VALUES (2, 'Bob', 87.0);"
    ).unwrap();
    let result = crate::executor::execute_query_columnar(&conn, "SELECT * FROM t ORDER BY id").unwrap();
    assert_eq!(result.row_count, 2);
    assert_eq!(result.columns.len(), 3);
    assert!(!result.truncated);
}

#[test]
fn test_execute_query_columnar_empty() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE t (id INTEGER, name TEXT)").unwrap();
    let result = crate::executor::execute_query_columnar(&conn, "SELECT * FROM t").unwrap();
    assert_eq!(result.row_count, 0);
    assert_eq!(result.columns.len(), 2);
    assert_eq!(result.column_data.len(), 2);
}

#[test]
fn test_execute_multi_columnar() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    let result = crate::executor::execute_multi_columnar(&conn, "SELECT 1; SELECT 2").unwrap();
    assert_eq!(result.results.len(), 2);
}
