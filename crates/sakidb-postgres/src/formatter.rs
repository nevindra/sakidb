use std::fmt::Write;

use sakidb_core::types::*;
use sakidb_core::SqlFormatter;

use crate::PostgresDriver;

fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// Write a CellValue in PostgreSQL COPY text format into the buffer.
fn write_copy_cell(buf: &mut String, cell: &CellValue) {
    match cell {
        CellValue::Null => buf.push_str("\\N"),
        CellValue::Bool(b) => {
            let _ = write!(buf, "{b}");
        }
        CellValue::Int(i) => {
            let _ = write!(buf, "{i}");
        }
        CellValue::Float(f) => {
            let _ = write!(buf, "{f}");
        }
        CellValue::Text(s) | CellValue::Json(s) | CellValue::Timestamp(s) => {
            for ch in s.chars() {
                match ch {
                    '\\' => buf.push_str("\\\\"),
                    '\t' => buf.push_str("\\t"),
                    '\n' => buf.push_str("\\n"),
                    '\r' => buf.push_str("\\r"),
                    _ => buf.push(ch),
                }
            }
        }
        CellValue::Bytes(b) => {
            buf.push_str("\\\\x");
            for byte in b {
                let _ = write!(buf, "{byte:02x}");
            }
        }
    }
}

impl SqlFormatter for PostgresDriver {
    fn format_ddl(&self, ctx: &DdlContext<'_>) -> Option<String> {
        let mut out = String::new();

        let _ = writeln!(out, "CREATE TABLE {} (", ctx.qualified_table);

        let mut col_defs: Vec<String> = Vec::new();
        for col in ctx.columns {
            let mut def = format!("    {} {}", quote_ident(&col.name), col.data_type);
            if !col.is_nullable {
                def.push_str(" NOT NULL");
            }
            if let Some(ref default) = col.default_value {
                let _ = write!(def, " DEFAULT {default}");
            }
            col_defs.push(def);
        }

        for uc in ctx.constraints {
            let cols: Vec<String> = uc.columns.iter().map(|c| quote_ident(c)).collect();
            if uc.is_primary {
                col_defs.push(format!(
                    "    CONSTRAINT {} PRIMARY KEY ({})",
                    quote_ident(&uc.constraint_name),
                    cols.join(", ")
                ));
            } else {
                col_defs.push(format!(
                    "    CONSTRAINT {} UNIQUE ({})",
                    quote_ident(&uc.constraint_name),
                    cols.join(", ")
                ));
            }
        }

        for fk in ctx.foreign_keys {
            let local_cols: Vec<String> = fk.columns.iter().map(|c| quote_ident(c)).collect();
            let foreign_cols: Vec<String> =
                fk.foreign_columns.iter().map(|c| quote_ident(c)).collect();
            col_defs.push(format!(
                "    CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {}.{} ({}) ON UPDATE {} ON DELETE {}",
                quote_ident(&fk.constraint_name),
                local_cols.join(", "),
                quote_ident(&fk.foreign_table_schema),
                quote_ident(&fk.foreign_table_name),
                foreign_cols.join(", "),
                fk.on_update,
                fk.on_delete
            ));
        }

        for cc in ctx.check_constraints {
            col_defs.push(format!(
                "    CONSTRAINT {} {}",
                quote_ident(&cc.constraint_name),
                cc.check_clause
            ));
        }

        let _ = writeln!(out, "{}\n);\n", col_defs.join(",\n"));

        for idx in ctx
            .indexes
            .iter()
            .filter(|i| i.table_name == ctx.table_name && !i.is_primary)
        {
            let unique = if idx.is_unique { "UNIQUE " } else { "" };
            let _ = writeln!(
                out,
                "CREATE {unique}INDEX {} ON {} USING {} ({});\n",
                quote_ident(&idx.name),
                ctx.qualified_table,
                idx.index_type,
                idx.columns
            );
        }

        for trig in ctx.triggers {
            let condition = trig
                .condition
                .as_ref()
                .map(|c| format!("\n    WHEN ({c})"))
                .unwrap_or_default();
            let _ = writeln!(
                out,
                "CREATE TRIGGER {} {} {} ON {}\n    FOR EACH {}{}\n    EXECUTE FUNCTION {}.{}();\n",
                quote_ident(&trig.name),
                trig.timing,
                trig.event,
                ctx.qualified_table,
                trig.for_each,
                condition,
                quote_ident(&trig.function_schema),
                quote_ident(&trig.function_name)
            );
        }

        Some(out)
    }

    fn format_data_header(&self, columns: &[ColumnDef], qualified_table: &str) -> Option<String> {
        let col_names: Vec<String> = columns.iter().map(|c| quote_ident(&c.name)).collect();
        Some(format!(
            "COPY {qualified_table} ({}) FROM stdin;\n",
            col_names.join(", ")
        ))
    }

    fn format_data_row(
        &self,
        columns: &[ColumnDef],
        cells: &[CellValue],
        _qualified_table: &str,
        buf: &mut String,
    ) {
        let num_cols = columns.len();
        for (col_idx, cell) in cells.iter().enumerate().take(num_cols) {
            if col_idx > 0 {
                buf.push('\t');
            }
            write_copy_cell(buf, cell);
        }
        buf.push('\n');
    }

    fn format_data_footer(&self) -> Option<String> {
        Some("\\.\n".to_string())
    }
}
