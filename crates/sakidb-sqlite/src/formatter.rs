use std::fmt::Write;

use sakidb_core::types::*;
use sakidb_core::SqlFormatter;

use crate::SqliteDriver;

fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn write_sqlite_literal(buf: &mut String, cell: &CellValue) {
    match cell {
        CellValue::Null => buf.push_str("NULL"),
        CellValue::Bool(b) => buf.push(if *b { '1' } else { '0' }),
        CellValue::Int(i) => {
            let _ = write!(buf, "{i}");
        }
        CellValue::Float(f) => {
            if f.is_finite() {
                let _ = write!(buf, "{f}");
            } else {
                buf.push_str("NULL");
            }
        }
        CellValue::Text(s) | CellValue::Json(s) | CellValue::Timestamp(s) => {
            buf.push('\'');
            for ch in s.chars() {
                if ch == '\'' {
                    buf.push('\'');
                }
                buf.push(ch);
            }
            buf.push('\'');
        }
        CellValue::Bytes(b) => {
            buf.push_str("X'");
            for byte in b {
                let _ = write!(buf, "{byte:02x}");
            }
            buf.push('\'');
        }
    }
}

impl SqlFormatter for SqliteDriver {
    fn format_ddl(&self, _ctx: &DdlContext<'_>) -> Option<String> {
        // SQLite DDL is best retrieved from sqlite_master (via get_create_table_sql)
        // rather than reconstructed from metadata. Return None — export.rs will
        // use the introspector's get_create_table_sql instead.
        None
    }

    fn format_data_header(
        &self,
        _columns: &[ColumnDef],
        _qualified_table: &str,
    ) -> Option<String> {
        None
    }

    fn format_data_row(
        &self,
        columns: &[ColumnDef],
        cells: &[CellValue],
        qualified_table: &str,
        buf: &mut String,
    ) {
        let num_cols = columns.len();
        let col_names: Vec<String> = columns.iter().map(|c| quote_ident(&c.name)).collect();

        let _ = write!(buf, "INSERT INTO {qualified_table} ({}) VALUES (", col_names.join(", "));
        for col_idx in 0..num_cols {
            if col_idx > 0 {
                buf.push_str(", ");
            }
            write_sqlite_literal(buf, &cells[col_idx]);
        }
        buf.push_str(");\n");
    }

    fn format_data_footer(&self) -> Option<String> {
        None
    }
}
