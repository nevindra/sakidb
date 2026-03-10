use sakidb_core::types::*;
use sakidb_core::SqlFormatter;

use crate::PostgresDriver;

fn driver() -> PostgresDriver {
    PostgresDriver::new()
}

fn col_info(name: &str, data_type: &str, nullable: bool, default: Option<&str>) -> ColumnInfo {
    ColumnInfo {
        name: name.to_string(),
        data_type: data_type.to_string(),
        is_nullable: nullable,
        default_value: default.map(|s| s.to_string()),
        is_primary_key: false,
    }
}

fn col_def(name: &str) -> ColumnDef {
    ColumnDef {
        name: name.to_string(),
        data_type: "text".to_string(),
    }
}

#[test]
fn format_data_header_copy_format() {
    let d = driver();
    let cols = vec![col_def("id"), col_def("name")];
    let header = d.format_data_header(&cols, "\"public\".\"users\"");
    assert_eq!(
        header.unwrap(),
        "COPY \"public\".\"users\" (\"id\", \"name\") FROM stdin;\n"
    );
}

#[test]
fn format_data_row_basic_types() {
    let d = driver();
    let cols = vec![col_def("a"), col_def("b"), col_def("c")];
    let cells = vec![
        CellValue::Int(42),
        CellValue::Text("hello".into()),
        CellValue::Null,
    ];
    let mut buf = String::new();
    d.format_data_row(&cols, &cells, "\"t\"", &mut buf);
    assert_eq!(buf, "42\thello\t\\N\n");
}

#[test]
fn format_data_row_escapes_special_chars() {
    let d = driver();
    let cols = vec![col_def("val")];
    let cells = vec![CellValue::Text("line1\nline2\ttab\\back".into())];
    let mut buf = String::new();
    d.format_data_row(&cols, &cells, "\"t\"", &mut buf);
    assert_eq!(buf, "line1\\nline2\\ttab\\\\back\n");
}

#[test]
fn format_data_row_bool() {
    let d = driver();
    let cols = vec![col_def("b")];
    let cells = vec![CellValue::Bool(true)];
    let mut buf = String::new();
    d.format_data_row(&cols, &cells, "\"t\"", &mut buf);
    assert_eq!(buf, "true\n");
}

#[test]
fn format_data_row_bytes() {
    let d = driver();
    let cols = vec![col_def("b")];
    let cells = vec![CellValue::Bytes(vec![0xca, 0xfe].into())];
    let mut buf = String::new();
    d.format_data_row(&cols, &cells, "\"t\"", &mut buf);
    assert_eq!(buf, "\\\\xcafe\n");
}

#[test]
fn format_data_footer() {
    let d = driver();
    assert_eq!(d.format_data_footer().unwrap(), "\\.\n");
}

#[test]
fn format_ddl_simple_table() {
    let d = driver();
    let cols = vec![
        col_info("id", "integer", false, None),
        col_info("name", "text", true, Some("'unnamed'")),
    ];
    let ddl = d
        .format_ddl(&DdlContext {
            columns: &cols, indexes: &[], constraints: &[], foreign_keys: &[],
            check_constraints: &[], triggers: &[], qualified_table: "\"public\".\"users\"", table_name: "users",
        })
        .unwrap();
    assert!(ddl.contains("CREATE TABLE \"public\".\"users\""));
    assert!(ddl.contains("\"id\" integer NOT NULL"));
    assert!(ddl.contains("\"name\" text DEFAULT 'unnamed'"));
}

#[test]
fn format_ddl_with_primary_key() {
    let d = driver();
    let cols = vec![col_info("id", "integer", false, None)];
    let constraints = vec![UniqueConstraintInfo {
        constraint_name: "users_pkey".to_string(),
        columns: vec!["id".to_string()],
        is_primary: true,
    }];
    let ddl = d
        .format_ddl(&DdlContext {
            columns: &cols, indexes: &[], constraints: &constraints, foreign_keys: &[],
            check_constraints: &[], triggers: &[], qualified_table: "\"users\"", table_name: "users",
        })
        .unwrap();
    assert!(ddl.contains("CONSTRAINT \"users_pkey\" PRIMARY KEY (\"id\")"));
}

#[test]
fn format_ddl_with_index_using() {
    let d = driver();
    let cols = vec![col_info("name", "text", true, None)];
    let indexes = vec![IndexInfo {
        name: "idx_name".to_string(),
        table_name: "users".to_string(),
        columns: "name".to_string(),
        is_unique: false,
        is_primary: false,
        index_type: "btree".to_string(),
    }];
    let ddl = d
        .format_ddl(&DdlContext {
            columns: &cols, indexes: &indexes, constraints: &[], foreign_keys: &[],
            check_constraints: &[], triggers: &[], qualified_table: "\"users\"", table_name: "users",
        })
        .unwrap();
    assert!(ddl.contains("CREATE INDEX \"idx_name\" ON \"users\" USING btree (name)"));
}
