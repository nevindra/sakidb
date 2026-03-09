use sakidb_core::types::*;
use sakidb_core::SqlFormatter;

use crate::SqliteDriver;

fn driver() -> SqliteDriver {
    SqliteDriver::new()
}

fn col_def(name: &str) -> ColumnDef {
    ColumnDef {
        name: name.to_string(),
        data_type: "text".to_string(),
    }
}

#[test]
fn format_ddl_returns_none() {
    let d = driver();
    let result = d.format_ddl(&[], &[], &[], &[], &[], &[], "\"users\"", "users");
    assert!(result.is_none(), "SQLite DDL should return None (use sqlite_master)");
}

#[test]
fn format_data_header_returns_none() {
    let d = driver();
    let cols = vec![col_def("id")];
    assert!(d.format_data_header(&cols, "\"users\"").is_none());
}

#[test]
fn format_data_footer_returns_none() {
    let d = driver();
    assert!(d.format_data_footer().is_none());
}

#[test]
fn format_data_row_insert_basic() {
    let d = driver();
    let cols = vec![col_def("id"), col_def("name")];
    let cells = vec![CellValue::Int(1), CellValue::Text("alice".into())];
    let mut buf = String::new();
    d.format_data_row(&cols, &cells, "\"users\"", &mut buf);
    assert_eq!(buf, "INSERT INTO \"users\" (\"id\", \"name\") VALUES (1, 'alice');\n");
}

#[test]
fn format_data_row_null() {
    let d = driver();
    let cols = vec![col_def("val")];
    let cells = vec![CellValue::Null];
    let mut buf = String::new();
    d.format_data_row(&cols, &cells, "\"t\"", &mut buf);
    assert!(buf.contains("NULL"));
}

#[test]
fn format_data_row_bool_as_int() {
    let d = driver();
    let cols = vec![col_def("flag")];
    let cells = vec![CellValue::Bool(true)];
    let mut buf = String::new();
    d.format_data_row(&cols, &cells, "\"t\"", &mut buf);
    assert!(buf.contains("1"), "SQLite bools should be 1/0");

    let mut buf2 = String::new();
    d.format_data_row(&cols, &[CellValue::Bool(false)], "\"t\"", &mut buf2);
    assert!(buf2.contains("0"));
}

#[test]
fn format_data_row_text_escaping() {
    let d = driver();
    let cols = vec![col_def("s")];
    let cells = vec![CellValue::Text("it's a test".into())];
    let mut buf = String::new();
    d.format_data_row(&cols, &cells, "\"t\"", &mut buf);
    assert!(buf.contains("'it''s a test'"), "Single quotes should be doubled");
}

#[test]
fn format_data_row_bytes_hex() {
    let d = driver();
    let cols = vec![col_def("data")];
    let cells = vec![CellValue::Bytes(vec![0xde, 0xad].into())];
    let mut buf = String::new();
    d.format_data_row(&cols, &cells, "\"t\"", &mut buf);
    assert!(buf.contains("X'dead'"), "Bytes should be X'hex' format");
}

#[test]
fn format_data_row_float() {
    let d = driver();
    let cols = vec![col_def("val")];
    let cells = vec![CellValue::Float(3.14)];
    let mut buf = String::new();
    d.format_data_row(&cols, &cells, "\"t\"", &mut buf);
    assert!(buf.contains("3.14"));
}

#[test]
fn format_data_row_non_finite_float() {
    let d = driver();
    let cols = vec![col_def("val")];
    let cells = vec![CellValue::Float(f64::INFINITY)];
    let mut buf = String::new();
    d.format_data_row(&cols, &cells, "\"t\"", &mut buf);
    assert!(buf.contains("NULL"), "Non-finite floats should become NULL");
}
