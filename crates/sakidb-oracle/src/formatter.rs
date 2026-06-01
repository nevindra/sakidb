use sakidb_core::{
    driver::SqlFormatter,
    types::{ColumnDef, CellValue, DdlContext},
};

pub struct OracleFormatter;

impl SqlFormatter for OracleFormatter {
    fn format_ddl(&self, ctx: &DdlContext<'_>) -> Option<String> {
        let mut ddl = String::new();

        ddl.push_str(&format!("CREATE TABLE {} (\n", ctx.qualified_table));

        let mut column_defs = Vec::new();
        for col in ctx.columns {
            let mut col_def = format!("    {} {}", col.name, col.data_type);
            if !col.is_nullable {
                col_def.push_str(" NOT NULL");
            }
            if let Some(default) = &col.default_value {
                col_def.push_str(&format!(" DEFAULT {}", default));
            }
            column_defs.push(col_def);
        }
        ddl.push_str(&column_defs.join(",\n"));

        let pk_columns: Vec<String> = ctx.columns
            .iter()
            .filter(|col| col.is_primary_key)
            .map(|col| col.name.clone())
            .collect();
        if !pk_columns.is_empty() {
            ddl.push_str(&format!(
                "\n    CONSTRAINT pk_{} PRIMARY KEY ({})",
                ctx.table_name,
                pk_columns.join(", ")
            ));
        }

        for constraint in ctx.constraints {
            if !constraint.is_primary {
                ddl.push_str(&format!(
                    "\n    CONSTRAINT {} UNIQUE ({})",
                    constraint.constraint_name,
                    constraint.columns.join(", ")
                ));
            }
        }

        for fk in ctx.foreign_keys {
            ddl.push_str(&format!(
                "\n    CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {} ({}) ON DELETE {}",
                fk.constraint_name,
                fk.columns.join(", "),
                fk.foreign_table_name,
                fk.foreign_columns.join(", "),
                fk.on_delete
            ));
        }

        for check in ctx.check_constraints {
            ddl.push_str(&format!(
                "\n    CONSTRAINT {} CHECK ({})",
                check.constraint_name,
                check.check_clause
            ));
        }

        ddl.push_str("\n);\n");

        for index in ctx.indexes {
            if !index.is_primary {
                let index_type = if index.is_unique { "UNIQUE " } else { "" };
                ddl.push_str(&format!(
                    "CREATE {}INDEX {} ON {} ({});\n",
                    index_type, index.name, ctx.qualified_table, index.columns
                ));
            }
        }

        for trigger in ctx.triggers {
            if trigger.is_enabled {
                ddl.push_str(&format!(
                    "CREATE OR REPLACE TRIGGER {}\n    {} {} ON {}\n    FOR EACH {}\nBEGIN\n    NULL;\nEND;\n",
                    trigger.name,
                    trigger.timing,
                    trigger.event,
                    trigger.table_name,
                    trigger.for_each
                ));
            }
        }

        Some(ddl)
    }

    fn format_data_header(&self, _columns: &[ColumnDef], _qualified_table: &str) -> Option<String> {
        None
    }

    fn format_data_row(
        &self,
        columns: &[ColumnDef],
        cells: &[CellValue],
        qualified_table: &str,
        buf: &mut String,
    ) {
        buf.push_str("INSERT INTO ");
        buf.push_str(qualified_table);
        buf.push_str(" (");

        for (i, col) in columns.iter().enumerate() {
            if i > 0 {
                buf.push(',');
            }
            buf.push_str(&col.name);
        }

        buf.push_str(") VALUES (");

        for (i, cell) in cells.iter().enumerate() {
            if i > 0 {
                buf.push_str(", ");
            }
            match cell {
                CellValue::Null => buf.push_str("NULL"),
                CellValue::Bool(b) => buf.push_str(if *b { "1" } else { "0" }),
                CellValue::Int(i) => buf.push_str(&i.to_string()),
                CellValue::Float(f) => buf.push_str(&f.to_string()),
                CellValue::Text(s) => {
                    buf.push('\'');
                    buf.push_str(&s.replace('\'', "''"));
                    buf.push('\'');
                }
                CellValue::Bytes(b) => {
                    buf.push_str("UTL_RAW.CAST_TO_VARCHAR2(HEXTORAW('");
                    buf.push_str(&hex::encode(b.as_ref()));
                    buf.push_str("'))");
                }
                CellValue::Json(j) => {
                    buf.push('\'');
                    buf.push_str(&j.replace('\'', "''"));
                    buf.push('\'');
                }
                CellValue::Timestamp(t) => {
                    buf.push_str("TO_TIMESTAMP('");
                    buf.push_str(t);
                    buf.push_str("', 'YYYY-MM-DD HH24:MI:SS.FF6')");
                }
            }
        }

        buf.push_str(");\n");
    }

    fn format_data_footer(&self) -> Option<String> {
        Some("COMMIT;\n".to_string())
    }
}
