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
