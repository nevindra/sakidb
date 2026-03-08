use std::collections::HashMap;

use rusqlite::Connection;
use tracing::debug;

use sakidb_core::types::*;
use sakidb_core::SakiError;

pub fn list_tables(conn: &Connection) -> Result<Vec<TableInfo>, SakiError> {
    debug!("listing tables");
    let mut stmt = conn
        .prepare(
            "SELECT name FROM sqlite_master
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
             ORDER BY name",
        )
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

    let tables = stmt
        .query_map([], |row| {
            Ok(TableInfo {
                name: row.get(0)?,
                row_count_estimate: None,
                size_bytes: None,
                is_partition: false,
                parent_table: None,
            })
        })
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

    debug!(count = tables.len(), "listed tables");
    Ok(tables)
}

pub fn list_columns(conn: &Connection, table: &str) -> Result<Vec<ColumnInfo>, SakiError> {
    debug!(table, "listing columns");
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info(\"{}\")", table.replace('"', "\"\"")))
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

    let columns = stmt
        .query_map([], |row| {
            let pk: i32 = row.get(5)?;
            let notnull: bool = row.get(3)?;
            Ok(ColumnInfo {
                name: row.get(1)?,
                data_type: row.get::<_, String>(2).unwrap_or_default(),
                is_nullable: !notnull,
                is_primary_key: pk > 0,
                default_value: row.get(4)?,
            })
        })
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

    Ok(columns)
}

pub fn list_views(conn: &Connection) -> Result<Vec<ViewInfo>, SakiError> {
    debug!("listing views");
    let mut stmt = conn
        .prepare(
            "SELECT name FROM sqlite_master
             WHERE type = 'view'
             ORDER BY name",
        )
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

    let views = stmt
        .query_map([], |row| {
            Ok(ViewInfo {
                name: row.get(0)?,
                is_updatable: false,
            })
        })
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

    Ok(views)
}

pub fn list_indexes(conn: &Connection, table: &str) -> Result<Vec<IndexInfo>, SakiError> {
    debug!(table, "listing indexes");

    let mut stmt = conn
        .prepare(&format!(
            "PRAGMA index_list(\"{}\")",
            table.replace('"', "\"\"")
        ))
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

    let index_list: Vec<(String, bool, String)> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?, // name
                row.get::<_, bool>(2)?,    // unique
                row.get::<_, String>(3)?,  // origin (c=CREATE INDEX, u=UNIQUE, pk=PRIMARY KEY)
            ))
        })
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

    let mut indexes = Vec::with_capacity(index_list.len());

    for (name, is_unique, origin) in index_list {
        // Get columns for this index
        let mut info_stmt = conn
            .prepare(&format!(
                "PRAGMA index_info(\"{}\")",
                name.replace('"', "\"\"")
            ))
            .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

        let cols: Vec<String> = info_stmt
            .query_map([], |row| row.get::<_, String>(2))
            .map_err(|e| SakiError::QueryFailed(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

        indexes.push(IndexInfo {
            name: name.clone(),
            table_name: table.to_string(),
            columns: cols.join(", "),
            is_unique,
            is_primary: origin == "pk",
            index_type: "btree".to_string(),
        });
    }

    Ok(indexes)
}

pub fn list_triggers(conn: &Connection, table: &str) -> Result<Vec<TriggerInfo>, SakiError> {
    debug!(table, "listing triggers");

    let mut stmt = conn
        .prepare(
            "SELECT name, tbl_name, sql FROM sqlite_master
             WHERE type = 'trigger' AND tbl_name = ?1
             ORDER BY name",
        )
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

    let triggers = stmt
        .query_map([table], |row| {
            let name: String = row.get(0)?;
            let tbl_name: String = row.get(1)?;
            let sql: Option<String> = row.get(2)?;

            // Parse timing and event from CREATE TRIGGER SQL
            let (timing, event) = sql
                .as_deref()
                .map(parse_trigger_sql)
                .unwrap_or(("UNKNOWN".to_string(), "UNKNOWN".to_string()));

            Ok(TriggerInfo {
                name,
                table_name: tbl_name,
                event,
                timing,
                for_each: "ROW".to_string(),
                function_name: String::new(),
                function_schema: String::new(),
                condition: None,
                is_enabled: true,
            })
        })
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

    Ok(triggers)
}

/// Parse timing (BEFORE/AFTER/INSTEAD OF) and event (INSERT/UPDATE/DELETE) from trigger SQL.
fn parse_trigger_sql(sql: &str) -> (String, String) {
    let upper = sql.to_uppercase();
    let timing = if upper.contains("BEFORE") {
        "BEFORE"
    } else if upper.contains("INSTEAD OF") {
        "INSTEAD OF"
    } else {
        "AFTER"
    }
    .to_string();

    let event = if upper.contains("INSERT") {
        "INSERT"
    } else if upper.contains("UPDATE") {
        "UPDATE"
    } else if upper.contains("DELETE") {
        "DELETE"
    } else {
        "UNKNOWN"
    }
    .to_string();

    (timing, event)
}

pub fn list_foreign_keys(conn: &Connection, table: &str) -> Result<Vec<ForeignKeyInfo>, SakiError> {
    debug!(table, "listing foreign keys");

    let mut stmt = conn
        .prepare(&format!(
            "PRAGMA foreign_key_list(\"{}\")",
            table.replace('"', "\"\"")
        ))
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

    // foreign_key_list columns: id, seq, table, from, to, on_update, on_delete, match
    // Group by id to handle multi-column foreign keys
    let mut fk_map: HashMap<i32, ForeignKeyInfo> = HashMap::new();

    let rows: Vec<(i32, String, String, String, String, String)> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i32>(0)?,    // id
                row.get::<_, String>(2)?,  // foreign table
                row.get::<_, String>(3)?,  // from column
                row.get::<_, String>(4)?,  // to column
                row.get::<_, String>(5)?,  // on_update
                row.get::<_, String>(6)?,  // on_delete
            ))
        })
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

    for (id, foreign_table, from_col, to_col, on_update, on_delete) in rows {
        let entry = fk_map.entry(id).or_insert_with(|| ForeignKeyInfo {
            constraint_name: format!("fk_{table}_{id}"),
            columns: Vec::new(),
            foreign_table_schema: "main".to_string(),
            foreign_table_name: foreign_table,
            foreign_columns: Vec::new(),
            on_update: on_update.clone(),
            on_delete: on_delete.clone(),
        });
        entry.columns.push(from_col);
        entry.foreign_columns.push(to_col);
    }

    let mut result: Vec<ForeignKeyInfo> = fk_map.into_values().collect();
    result.sort_by(|a, b| a.constraint_name.cmp(&b.constraint_name));
    Ok(result)
}

pub fn list_check_constraints(
    conn: &Connection,
    table: &str,
) -> Result<Vec<CheckConstraintInfo>, SakiError> {
    // SQLite doesn't expose check constraints via a pragma.
    // Parse them from the CREATE TABLE SQL.
    let sql: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

    let Some(create_sql) = sql else {
        return Ok(vec![]);
    };

    let mut constraints = Vec::new();
    let upper = create_sql.to_uppercase();
    let mut idx = 0;
    let mut check_num = 0;

    while let Some(pos) = upper[idx..].find("CHECK") {
        let abs_pos = idx + pos;
        // Find the opening paren
        if let Some(paren_start) = create_sql[abs_pos..].find('(') {
            let start = abs_pos + paren_start;
            let mut depth = 0;
            let mut end = start;
            for (i, ch) in create_sql[start..].char_indices() {
                match ch {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            end = start + i + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if end > start {
                check_num += 1;
                constraints.push(CheckConstraintInfo {
                    constraint_name: format!("check_{check_num}"),
                    check_clause: create_sql[start..end].to_string(),
                });
            }
            idx = end;
        } else {
            break;
        }
    }

    Ok(constraints)
}

pub fn list_unique_constraints(
    conn: &Connection,
    table: &str,
) -> Result<Vec<UniqueConstraintInfo>, SakiError> {
    let indexes = list_indexes(conn, table)?;

    Ok(indexes
        .into_iter()
        .filter(|idx| idx.is_unique || idx.is_primary)
        .map(|idx| UniqueConstraintInfo {
            constraint_name: idx.name,
            columns: idx.columns.split(", ").map(|s| s.to_string()).collect(),
            is_primary: idx.is_primary,
        })
        .collect())
}

pub fn get_create_table_sql(conn: &Connection, table: &str) -> Result<String, SakiError> {
    let sql: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

    // Also get associated indexes
    let mut stmt = conn
        .prepare(
            "SELECT sql FROM sqlite_master
             WHERE type = 'index' AND tbl_name = ?1 AND sql IS NOT NULL
             ORDER BY name",
        )
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

    let index_sqls: Vec<String> = stmt
        .query_map([table], |row| row.get::<_, String>(0))
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

    let mut result = format!("{sql};\n");
    for idx_sql in index_sqls {
        result.push_str(&format!("\n{idx_sql};\n"));
    }

    Ok(result)
}

pub fn get_erd_data(conn: &Connection) -> Result<ErdData, SakiError> {
    debug!("loading ERD data");

    let tables = list_tables(conn)?;

    let mut columns: HashMap<String, Vec<ColumnInfo>> = HashMap::new();
    let mut foreign_keys: HashMap<String, Vec<ForeignKeyInfo>> = HashMap::new();

    for table in &tables {
        columns.insert(table.name.clone(), list_columns(conn, &table.name)?);
        let fks = list_foreign_keys(conn, &table.name)?;
        if !fks.is_empty() {
            foreign_keys.insert(table.name.clone(), fks);
        }
    }

    Ok(ErdData {
        tables,
        columns,
        foreign_keys,
    })
}

pub fn get_schema_completion_data(
    conn: &Connection,
) -> Result<HashMap<String, Vec<String>>, SakiError> {
    let mut result: HashMap<String, Vec<String>> = HashMap::new();

    // Get all tables and views
    let mut stmt = conn
        .prepare(
            "SELECT name FROM sqlite_master
             WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%'
             ORDER BY name",
        )
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

    let names: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

    for name in names {
        let cols = list_columns(conn, &name)?;
        result.insert(name, cols.into_iter().map(|c| c.name).collect());
    }

    Ok(result)
}

pub fn get_completion_bundle(conn: &Connection) -> Result<CompletionBundle, SakiError> {
    let mut stmt = conn
        .prepare(
            "SELECT name, type FROM sqlite_master
             WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%'
             ORDER BY name",
        )
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

    let tables = stmt
        .query_map([], |row| {
            Ok(CompletionTable {
                name: row.get(0)?,
                kind: row.get(1)?,
            })
        })
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| SakiError::QueryFailed(e.to_string()))?;

    // SQLite doesn't have user-defined functions accessible via metadata
    Ok(CompletionBundle {
        tables,
        functions: vec![],
    })
}

pub fn get_table_columns_for_completion(
    conn: &Connection,
    table: &str,
) -> Result<Vec<CompletionColumn>, SakiError> {
    let cols = list_columns(conn, table)?;
    Ok(cols
        .into_iter()
        .map(|c| CompletionColumn {
            name: c.name,
            data_type: c.data_type,
            is_primary_key: c.is_primary_key,
            is_nullable: c.is_nullable,
        })
        .collect())
}
