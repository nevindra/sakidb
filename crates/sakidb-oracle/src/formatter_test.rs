#[cfg(test)]
mod tests {
    use crate::formatter::OracleFormatter;
    use sakidb_core::driver::SqlFormatter;
    use sakidb_core::types::{ColumnDef, ColumnInfo, CellValue, DdlContext, UniqueConstraintInfo};

    fn make_col(name: &str, data_type: &str) -> ColumnInfo {
        ColumnInfo {
            name: name.to_string(),
            data_type: data_type.to_string(),
            is_nullable: true,
            is_primary_key: false,
            default_value: None,
        }
    }

    #[test]
    fn test_format_ddl_simple_table() {
        let columns = vec![
            make_col("id", "NUMBER"),
            make_col("name", "VARCHAR2(100)"),
            make_col("created_at", "TIMESTAMP"),
        ];

        let ctx = DdlContext {
            columns: &columns,
            indexes: &[],
            constraints: &[],
            foreign_keys: &[],
            check_constraints: &[],
            triggers: &[],
            qualified_table: "test.users",
            table_name: "users",
        };

        let formatter = OracleFormatter;
        let ddl = formatter.format_ddl(&ctx).unwrap();

        assert!(ddl.contains("CREATE TABLE test.users"), "ddl: {}", ddl);
        assert!(ddl.contains("id NUMBER"), "ddl: {}", ddl);
        assert!(ddl.contains("name VARCHAR2(100)"), "ddl: {}", ddl);
        assert!(ddl.contains("created_at TIMESTAMP"), "ddl: {}", ddl);
        assert!(ddl.ends_with(";\n") || ddl.contains(";\n"), "ddl: {}", ddl);
    }

    #[test]
    fn test_format_ddl_with_constraints() {
        let columns = vec![
            make_col("id", "NUMBER"),
            make_col("email", "VARCHAR2(255)"),
        ];

        let constraints = vec![UniqueConstraintInfo {
            constraint_name: "uk_email".to_string(),
            columns: vec!["email".to_string()],
            is_primary: false,
        }];

        let ctx = DdlContext {
            columns: &columns,
            indexes: &[],
            constraints: &constraints,
            foreign_keys: &[],
            check_constraints: &[],
            triggers: &[],
            qualified_table: "test.users",
            table_name: "users",
        };

        let formatter = OracleFormatter;
        let ddl = formatter.format_ddl(&ctx).unwrap();

        assert!(ddl.contains("CREATE TABLE test.users"), "ddl: {}", ddl);
        assert!(ddl.contains("CONSTRAINT uk_email UNIQUE (email)"), "ddl: {}", ddl);
    }

    #[test]
    fn test_format_data_row() {
        let columns = vec![
            ColumnDef { name: "id".to_string(), data_type: "NUMBER".to_string() },
            ColumnDef { name: "name".to_string(), data_type: "VARCHAR2(100)".to_string() },
            ColumnDef { name: "active".to_string(), data_type: "NUMBER(1)".to_string() },
        ];

        let cells = vec![
            CellValue::Int(42),
            CellValue::Text("John O'Connor".to_string().into_boxed_str()),
            CellValue::Bool(true),
        ];

        let mut buf = String::new();
        let formatter = OracleFormatter;
        formatter.format_data_row(&columns, &cells, "test.users", &mut buf);

        assert!(buf.contains("INSERT INTO test.users"), "buf: {}", buf);
        assert!(buf.contains("(id,name,active)") || buf.contains("(id, name, active)") || buf.contains("id"), "buf: {}", buf);
        assert!(buf.contains("42"), "buf: {}", buf);
        assert!(buf.contains("John O''Connor"), "buf: {}", buf);
    }

    #[test]
    fn test_format_data_row_with_null() {
        let columns = vec![
            ColumnDef { name: "id".to_string(), data_type: "NUMBER".to_string() },
            ColumnDef { name: "name".to_string(), data_type: "VARCHAR2(100)".to_string() },
        ];

        let cells = vec![CellValue::Int(42), CellValue::Null];

        let mut buf = String::new();
        let formatter = OracleFormatter;
        formatter.format_data_row(&columns, &cells, "test.users", &mut buf);

        assert!(buf.contains("INSERT INTO test.users"), "buf: {}", buf);
        assert!(buf.contains("NULL"), "buf: {}", buf);
        assert!(buf.contains("42"), "buf: {}", buf);
    }

    #[test]
    fn test_format_data_row_with_bytes() {
        let columns = vec![
            ColumnDef { name: "id".to_string(), data_type: "NUMBER".to_string() },
            ColumnDef { name: "data".to_string(), data_type: "BLOB".to_string() },
        ];

        let cells = vec![
            CellValue::Int(42),
            CellValue::Bytes(vec![0x48, 0x65, 0x6C, 0x6C, 0x6F].into_boxed_slice()),
        ];

        let mut buf = String::new();
        let formatter = OracleFormatter;
        formatter.format_data_row(&columns, &cells, "test.users", &mut buf);

        assert!(buf.contains("INSERT INTO test.users"), "buf: {}", buf);
        assert!(buf.contains("UTL_RAW"), "buf: {}", buf);
        // hex::encode gives lowercase
        assert!(buf.contains("48656c6c6f") || buf.contains("48656C6C6F"), "buf: {}", buf);
    }

    #[test]
    fn test_format_data_row_with_timestamp() {
        let columns = vec![
            ColumnDef { name: "id".to_string(), data_type: "NUMBER".to_string() },
            ColumnDef { name: "created_at".to_string(), data_type: "TIMESTAMP".to_string() },
        ];

        let cells = vec![
            CellValue::Int(42),
            CellValue::Timestamp("2023-12-25 10:30:45.123456".to_string().into_boxed_str()),
        ];

        let mut buf = String::new();
        let formatter = OracleFormatter;
        formatter.format_data_row(&columns, &cells, "test.users", &mut buf);

        assert!(buf.contains("INSERT INTO test.users"), "buf: {}", buf);
        assert!(buf.contains("TO_TIMESTAMP('2023-12-25 10:30:45.123456'"), "buf: {}", buf);
    }

    #[test]
    fn test_format_data_header() {
        let columns = vec![ColumnDef { name: "id".to_string(), data_type: "NUMBER".to_string() }];
        let formatter = OracleFormatter;
        let header = formatter.format_data_header(&columns, "test.users");
        assert!(header.is_none());
    }

    #[test]
    fn test_format_data_footer() {
        let formatter = OracleFormatter;
        let footer = formatter.format_data_footer();
        assert_eq!(footer.unwrap(), "COMMIT;\n");
    }
}
