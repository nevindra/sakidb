use std::collections::HashMap;
use std::fmt::Write;

use deadpool_postgres::Pool;
use tracing::debug;

use sakidb_core::types::*;
use sakidb_core::SakiError;

use crate::executor::{format_pg_error, format_pool_error};

/// Escape a SQL identifier for use in DDL: doubles internal `"` characters.
fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

pub async fn list_databases(pool: &Pool) -> Result<Vec<DatabaseInfo>, SakiError> {
    let client = pool
        .get()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;

    let rows = client
        .query(
            "SELECT datname, datistemplate
             FROM pg_database
             ORDER BY datname",
            &[],
        )
        .await
        .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;

    let result: Vec<DatabaseInfo> = rows
        .iter()
        .map(|r| DatabaseInfo {
            name: r.get("datname"),
            is_template: r.get("datistemplate"),
        })
        .collect();
    debug!(count = result.len(), "listed databases");
    Ok(result)
}

pub async fn list_schemas(pool: &Pool) -> Result<Vec<SchemaInfo>, SakiError> {
    debug!("listing schemas");
    let client = pool
        .get()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;

    let rows = client
        .query(
            "SELECT schema_name FROM information_schema.schemata
             WHERE schema_name NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
               AND schema_name NOT LIKE 'pg_temp_%'
               AND schema_name NOT LIKE 'pg_toast_temp_%'
             ORDER BY schema_name",
            &[],
        )
        .await
        .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;

    Ok(rows
        .iter()
        .map(|r| SchemaInfo {
            name: r.get("schema_name"),
        })
        .collect())
}

pub async fn list_tables(pool: &Pool, schema: &str) -> Result<Vec<TableInfo>, SakiError> {
    debug!(schema, "listing tables");
    let client = pool
        .get()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;

    let rows = client
        .query(
            "SELECT c.relname AS table_name,
                    c.reltuples::bigint AS row_estimate,
                    pg_total_relation_size(c.oid)::bigint AS size_bytes,
                    c.relispartition AS is_partition,
                    p.relname AS parent_table
             FROM pg_catalog.pg_class c
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
             LEFT JOIN pg_catalog.pg_inherits i ON i.inhrelid = c.oid
             LEFT JOIN pg_catalog.pg_class p ON p.oid = i.inhparent
             WHERE n.nspname = $1
               AND c.relkind IN ('r', 'p')
               AND c.relname NOT LIKE 'pg_%'
             ORDER BY c.relname",
            &[&schema],
        )
        .await
        .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;

    Ok(rows
        .iter()
        .map(|r| TableInfo {
            name: r.get("table_name"),
            row_count_estimate: r.get("row_estimate"),
            size_bytes: r.get("size_bytes"),
            is_partition: r.get("is_partition"),
            parent_table: r.get("parent_table"),
        })
        .collect())
}

/// Fetch all table names and their column names in a single query.
/// Returns a map of table_name → [column_name, ...].
/// Used for editor autocompletion — avoids N+1 IPC calls.
pub async fn get_schema_completion_data(
    pool: &Pool,
    schema: &str,
) -> Result<HashMap<String, Vec<String>>, SakiError> {
    let client = pool
        .get()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;

    let rows = client
        .query(
            "SELECT c.relname AS table_name, a.attname AS column_name
             FROM pg_catalog.pg_class c
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
             JOIN pg_catalog.pg_attribute a ON a.attrelid = c.oid
             WHERE n.nspname = $1
               AND c.relkind IN ('r', 'p', 'v', 'm')
               AND a.attnum > 0
               AND NOT a.attisdropped
             ORDER BY c.relname, a.attnum",
            &[&schema],
        )
        .await
        .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;

    let mut result: HashMap<String, Vec<String>> = HashMap::new();
    for row in &rows {
        let table: String = row.get("table_name");
        let column: String = row.get("column_name");
        result.entry(table).or_default().push(column);
    }
    Ok(result)
}

/// Fetch table/view/matview names + all functions with signatures in a single operation.
/// Used for the "eager" part of the hybrid editor autocompletion strategy.
pub async fn get_completion_bundle(
    pool: &Pool,
    schema: &str,
) -> Result<CompletionBundle, SakiError> {
    let client = pool
        .get()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;

    let params: &[&(dyn tokio_postgres::types::ToSql + Sync)] = &[&schema];
    let (table_rows, func_rows) = tokio::try_join!(
        client.query(
            "SELECT c.relname AS name,
                    CASE c.relkind
                        WHEN 'r' THEN 'table'
                        WHEN 'p' THEN 'table'
                        WHEN 'v' THEN 'view'
                        WHEN 'm' THEN 'materialized_view'
                    END AS kind
             FROM pg_catalog.pg_class c
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
             WHERE n.nspname = $1
               AND c.relkind IN ('r', 'p', 'v', 'm')
               AND c.relname NOT LIKE 'pg_%'
             ORDER BY c.relname",
            params,
        ),
        client.query(
            "SELECT p.proname AS func_name,
                    CASE p.prokind
                        WHEN 'f' THEN 'function'
                        WHEN 'p' THEN 'procedure'
                        WHEN 'a' THEN 'aggregate'
                        WHEN 'w' THEN 'window'
                        ELSE 'function'
                    END AS func_kind,
                    pg_catalog.pg_get_function_result(p.oid) AS return_type,
                    pg_catalog.pg_get_function_identity_arguments(p.oid) AS argument_types,
                    l.lanname AS language
             FROM pg_catalog.pg_proc p
             JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace
             JOIN pg_catalog.pg_language l ON l.oid = p.prolang
             WHERE n.nspname = $1
               AND p.proname NOT LIKE 'pg_%'
             ORDER BY p.proname",
            params,
        ),
    )
    .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;

    let tables: Vec<CompletionTable> = table_rows
        .iter()
        .map(|r| CompletionTable {
            name: r.get("name"),
            kind: r.get("kind"),
        })
        .collect();

    let functions: Vec<FunctionInfo> = func_rows
        .iter()
        .map(|r| FunctionInfo {
            name: r.get("func_name"),
            kind: r.get("func_kind"),
            return_type: r.get::<_, Option<String>>("return_type").unwrap_or_default(),
            argument_types: r.get::<_, Option<String>>("argument_types").unwrap_or_default(),
            language: r.get("language"),
        })
        .collect();

    debug!(tables = tables.len(), functions = functions.len(), "completion bundle loaded");
    Ok(CompletionBundle { tables, functions })
}

/// Fetch columns for a single table with type info, for lazy per-table completion.
pub async fn get_table_columns_for_completion(
    pool: &Pool,
    schema: &str,
    table: &str,
) -> Result<Vec<CompletionColumn>, SakiError> {
    let client = pool
        .get()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;

    let rows = client
        .query(
            "SELECT a.attname AS column_name,
                    pg_catalog.format_type(a.atttypid, a.atttypmod) AS data_type,
                    a.attnotnull AS not_null,
                    COALESCE(
                        (SELECT true FROM pg_catalog.pg_index i
                         WHERE i.indrelid = a.attrelid
                           AND i.indisprimary
                           AND a.attnum = ANY(i.indkey)),
                        false
                    ) AS is_pk
             FROM pg_catalog.pg_attribute a
             JOIN pg_catalog.pg_class c ON c.oid = a.attrelid
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
             WHERE n.nspname = $1
               AND c.relname = $2
               AND a.attnum > 0
               AND NOT a.attisdropped
             ORDER BY a.attnum",
            &[&schema, &table],
        )
        .await
        .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;

    Ok(rows
        .iter()
        .map(|r| {
            let not_null: bool = r.get("not_null");
            CompletionColumn {
                name: r.get("column_name"),
                data_type: r.get("data_type"),
                is_primary_key: r.get("is_pk"),
                is_nullable: !not_null,
            }
        })
        .collect())
}

pub async fn list_views(pool: &Pool, schema: &str) -> Result<Vec<ViewInfo>, SakiError> {
    let client = pool
        .get()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;

    let rows = client
        .query(
            "SELECT v.table_name AS view_name,
                    v.is_updatable = 'YES' AS is_updatable
             FROM information_schema.views v
             WHERE v.table_schema = $1
             ORDER BY v.table_name",
            &[&schema],
        )
        .await
        .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;

    Ok(rows
        .iter()
        .map(|r| ViewInfo {
            name: r.get("view_name"),
            is_updatable: r.get("is_updatable"),
        })
        .collect())
}

pub async fn list_materialized_views(
    pool: &Pool,
    schema: &str,
) -> Result<Vec<MaterializedViewInfo>, SakiError> {
    let client = pool
        .get()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;

    let rows = client
        .query(
            "SELECT c.relname AS matview_name,
                    c.reltuples::bigint AS row_estimate,
                    c.relispopulated AS is_populated
             FROM pg_catalog.pg_class c
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
             WHERE n.nspname = $1
               AND c.relkind = 'm'
             ORDER BY c.relname",
            &[&schema],
        )
        .await
        .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;

    Ok(rows
        .iter()
        .map(|r| MaterializedViewInfo {
            name: r.get("matview_name"),
            row_count_estimate: r.get("row_estimate"),
            is_populated: r.get("is_populated"),
        })
        .collect())
}

pub async fn list_functions(pool: &Pool, schema: &str) -> Result<Vec<FunctionInfo>, SakiError> {
    let client = pool
        .get()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;

    let rows = client
        .query(
            "SELECT p.proname AS func_name,
                    CASE p.prokind
                        WHEN 'f' THEN 'function'
                        WHEN 'p' THEN 'procedure'
                        WHEN 'a' THEN 'aggregate'
                        WHEN 'w' THEN 'window'
                        ELSE 'function'
                    END AS func_kind,
                    pg_catalog.pg_get_function_result(p.oid) AS return_type,
                    pg_catalog.pg_get_function_identity_arguments(p.oid) AS argument_types,
                    l.lanname AS language
             FROM pg_catalog.pg_proc p
             JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace
             JOIN pg_catalog.pg_language l ON l.oid = p.prolang
             WHERE n.nspname = $1
               AND p.proname NOT LIKE 'pg_%'
             ORDER BY p.proname",
            &[&schema],
        )
        .await
        .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;

    Ok(rows
        .iter()
        .map(|r| FunctionInfo {
            name: r.get("func_name"),
            kind: r.get("func_kind"),
            return_type: r.get::<_, Option<String>>("return_type").unwrap_or_default(),
            argument_types: r.get::<_, Option<String>>("argument_types").unwrap_or_default(),
            language: r.get("language"),
        })
        .collect())
}

pub async fn list_sequences(pool: &Pool, schema: &str) -> Result<Vec<SequenceInfo>, SakiError> {
    let client = pool
        .get()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;

    let rows = client
        .query(
            "SELECT s.sequencename AS seq_name,
                    s.data_type,
                    s.last_value
             FROM pg_catalog.pg_sequences s
             WHERE s.schemaname = $1
             ORDER BY s.sequencename",
            &[&schema],
        )
        .await
        .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;

    Ok(rows
        .iter()
        .map(|r| SequenceInfo {
            name: r.get("seq_name"),
            data_type: r.get("data_type"),
            last_value: r.get("last_value"),
        })
        .collect())
}

pub async fn list_indexes(pool: &Pool, schema: &str) -> Result<Vec<IndexInfo>, SakiError> {
    let client = pool
        .get()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;

    let rows = client
        .query(
            "SELECT i.indexname AS index_name,
                    i.tablename AS table_name,
                    a.attname AS column_names,
                    ix.indisunique AS is_unique,
                    ix.indisprimary AS is_primary,
                    am.amname AS index_type
             FROM pg_catalog.pg_indexes i
             JOIN pg_catalog.pg_class c ON c.relname = i.indexname
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace AND n.nspname = i.schemaname
             JOIN pg_catalog.pg_index ix ON ix.indexrelid = c.oid
             JOIN pg_catalog.pg_am am ON am.oid = c.relam
             LEFT JOIN LATERAL (
                 SELECT string_agg(a2.attname, ', ' ORDER BY array_position(ix.indkey, a2.attnum)) AS attname
                 FROM pg_catalog.pg_attribute a2
                 WHERE a2.attrelid = ix.indrelid
                   AND a2.attnum = ANY(ix.indkey)
                   AND a2.attnum > 0
             ) a ON true
             WHERE i.schemaname = $1
             ORDER BY i.tablename, i.indexname",
            &[&schema],
        )
        .await
        .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;

    Ok(rows
        .iter()
        .map(|r| IndexInfo {
            name: r.get("index_name"),
            table_name: r.get("table_name"),
            columns: r.get::<_, Option<String>>("column_names").unwrap_or_default(),
            is_unique: r.get("is_unique"),
            is_primary: r.get("is_primary"),
            index_type: r.get::<_, Option<String>>("index_type").unwrap_or_default(),
        })
        .collect())
}

pub async fn list_foreign_tables(
    pool: &Pool,
    schema: &str,
) -> Result<Vec<ForeignTableInfo>, SakiError> {
    let client = pool
        .get()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;

    let rows = client
        .query(
            "SELECT ft.foreign_table_name AS table_name,
                    ft.foreign_server_name AS server_name
             FROM information_schema.foreign_tables ft
             WHERE ft.foreign_table_schema = $1
             ORDER BY ft.foreign_table_name",
            &[&schema],
        )
        .await
        .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;

    Ok(rows
        .iter()
        .map(|r| ForeignTableInfo {
            name: r.get("table_name"),
            server_name: r.get("server_name"),
        })
        .collect())
}

pub async fn list_columns(
    pool: &Pool,
    schema: &str,
    table: &str,
) -> Result<Vec<ColumnInfo>, SakiError> {
    let client = pool
        .get()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;

    let rows = client
        .query(
            "SELECT c.column_name, c.data_type, c.is_nullable, c.column_default,
                    COALESCE(pk.is_pk, false) AS is_pk
             FROM information_schema.columns c
             LEFT JOIN (
                 SELECT kcu.column_name, true AS is_pk
                 FROM information_schema.table_constraints tc
                 JOIN information_schema.key_column_usage kcu
                     ON tc.constraint_name = kcu.constraint_name
                     AND tc.table_schema = kcu.table_schema
                 WHERE tc.constraint_type = 'PRIMARY KEY'
                     AND tc.table_schema = $1
                     AND tc.table_name = $2
             ) pk ON pk.column_name = c.column_name
             WHERE c.table_schema = $1 AND c.table_name = $2
             ORDER BY c.ordinal_position",
            &[&schema, &table],
        )
        .await
        .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;

    Ok(rows
        .iter()
        .map(|r| ColumnInfo {
            name: r.get("column_name"),
            data_type: r.get("data_type"),
            is_nullable: r.get::<_, String>("is_nullable") == "YES",
            is_primary_key: r.get("is_pk"),
            default_value: r.get("column_default"),
        })
        .collect())
}

pub async fn list_triggers(
    pool: &Pool,
    schema: &str,
    table: &str,
) -> Result<Vec<TriggerInfo>, SakiError> {
    let client = pool
        .get()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;

    let rows = client
        .query(
            "SELECT t.tgname AS trigger_name,
                    c.relname AS table_name,
                    string_agg(
                        CASE em.event
                            WHEN 'INSERT' THEN 'INSERT'
                            WHEN 'UPDATE' THEN 'UPDATE'
                            WHEN 'DELETE' THEN 'DELETE'
                            WHEN 'TRUNCATE' THEN 'TRUNCATE'
                        END, ', '
                    ) AS event,
                    CASE
                        WHEN t.tgtype::int & 2 > 0 THEN 'BEFORE'
                        WHEN t.tgtype::int & 64 > 0 THEN 'INSTEAD OF'
                        ELSE 'AFTER'
                    END AS timing,
                    CASE WHEN t.tgtype::int & 1 > 0 THEN 'ROW' ELSE 'STATEMENT' END AS for_each,
                    p.proname AS function_name,
                    pn.nspname AS function_schema,
                    pg_catalog.pg_get_triggerdef(t.oid, true) AS trigger_def,
                    t.tgenabled != 'D' AS is_enabled
             FROM pg_catalog.pg_trigger t
             JOIN pg_catalog.pg_class c ON c.oid = t.tgrelid
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
             JOIN pg_catalog.pg_proc p ON p.oid = t.tgfoid
             JOIN pg_catalog.pg_namespace pn ON pn.oid = p.pronamespace
             LEFT JOIN LATERAL unnest(ARRAY[
                 CASE WHEN t.tgtype::int & 4 > 0 THEN 'INSERT' END,
                 CASE WHEN t.tgtype::int & 8 > 0 THEN 'DELETE' END,
                 CASE WHEN t.tgtype::int & 16 > 0 THEN 'UPDATE' END,
                 CASE WHEN t.tgtype::int & 32 > 0 THEN 'TRUNCATE' END
             ]) AS em(event) ON em.event IS NOT NULL
             WHERE n.nspname = $1
               AND c.relname = $2
               AND NOT t.tgisinternal
             GROUP BY t.tgname, c.relname, t.tgtype, p.proname, pn.nspname, t.oid, t.tgenabled
             ORDER BY t.tgname",
            &[&schema, &table],
        )
        .await
        .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;

    Ok(rows
        .iter()
        .map(|r| {
            let trigger_def: String = r.get::<_, Option<String>>("trigger_def").unwrap_or_default();
            let condition = trigger_def
                .find(" WHEN (")
                .map(|start| {
                    let after = &trigger_def[start + 7..];
                    after.find(") EXECUTE").map(|end| after[..end].to_string())
                })
                .flatten();

            TriggerInfo {
                name: r.get("trigger_name"),
                table_name: r.get("table_name"),
                event: r.get::<_, Option<String>>("event").unwrap_or_default(),
                timing: r.get("timing"),
                for_each: r.get("for_each"),
                function_name: r.get("function_name"),
                function_schema: r.get("function_schema"),
                condition,
                is_enabled: r.get("is_enabled"),
            }
        })
        .collect())
}

pub async fn list_foreign_keys(
    pool: &Pool,
    schema: &str,
    table: &str,
) -> Result<Vec<ForeignKeyInfo>, SakiError> {
    let client = pool
        .get()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;

    let rows = client
        .query(
            "SELECT
                 con.conname AS constraint_name,
                 array_agg(a.attname ORDER BY u.attposition) AS columns,
                 fn.nspname AS foreign_table_schema,
                 fc.relname AS foreign_table_name,
                 array_agg(fa.attname ORDER BY u.attposition) AS foreign_columns,
                 CASE con.confupdtype
                     WHEN 'a' THEN 'NO ACTION'
                     WHEN 'r' THEN 'RESTRICT'
                     WHEN 'c' THEN 'CASCADE'
                     WHEN 'n' THEN 'SET NULL'
                     WHEN 'd' THEN 'SET DEFAULT'
                     ELSE 'NO ACTION'
                 END AS on_update,
                 CASE con.confdeltype
                     WHEN 'a' THEN 'NO ACTION'
                     WHEN 'r' THEN 'RESTRICT'
                     WHEN 'c' THEN 'CASCADE'
                     WHEN 'n' THEN 'SET NULL'
                     WHEN 'd' THEN 'SET DEFAULT'
                     ELSE 'NO ACTION'
                 END AS on_delete
             FROM pg_catalog.pg_constraint con
             JOIN pg_catalog.pg_class c ON c.oid = con.conrelid
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
             JOIN pg_catalog.pg_class fc ON fc.oid = con.confrelid
             JOIN pg_catalog.pg_namespace fn ON fn.oid = fc.relnamespace
             JOIN LATERAL unnest(con.conkey, con.confkey) WITH ORDINALITY AS u(local_attnum, foreign_attnum, attposition) ON true
             JOIN pg_catalog.pg_attribute a ON a.attrelid = con.conrelid AND a.attnum = u.local_attnum
             JOIN pg_catalog.pg_attribute fa ON fa.attrelid = con.confrelid AND fa.attnum = u.foreign_attnum
             WHERE con.contype = 'f'
               AND n.nspname = $1
               AND c.relname = $2
             GROUP BY con.conname, fn.nspname, fc.relname, con.confupdtype, con.confdeltype
             ORDER BY con.conname",
            &[&schema, &table],
        )
        .await
        .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;

    Ok(rows
        .iter()
        .map(|r| ForeignKeyInfo {
            constraint_name: r.get("constraint_name"),
            columns: r.get::<_, Vec<String>>("columns"),
            foreign_table_schema: r.get("foreign_table_schema"),
            foreign_table_name: r.get("foreign_table_name"),
            foreign_columns: r.get::<_, Vec<String>>("foreign_columns"),
            on_update: r.get("on_update"),
            on_delete: r.get("on_delete"),
        })
        .collect())
}

pub async fn list_check_constraints(
    pool: &Pool,
    schema: &str,
    table: &str,
) -> Result<Vec<CheckConstraintInfo>, SakiError> {
    let client = pool
        .get()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;

    let rows = client
        .query(
            "SELECT con.conname AS constraint_name,
                    pg_catalog.pg_get_constraintdef(con.oid) AS check_clause
             FROM pg_catalog.pg_constraint con
             JOIN pg_catalog.pg_class c ON c.oid = con.conrelid
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
             WHERE con.contype = 'c'
               AND n.nspname = $1
               AND c.relname = $2
             ORDER BY con.conname",
            &[&schema, &table],
        )
        .await
        .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;

    Ok(rows
        .iter()
        .map(|r| CheckConstraintInfo {
            constraint_name: r.get("constraint_name"),
            check_clause: r.get("check_clause"),
        })
        .collect())
}

pub async fn list_unique_constraints(
    pool: &Pool,
    schema: &str,
    table: &str,
) -> Result<Vec<UniqueConstraintInfo>, SakiError> {
    let client = pool
        .get()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;

    let rows = client
        .query(
            "SELECT con.conname AS constraint_name,
                    array_agg(a.attname ORDER BY u.ord) AS columns,
                    con.contype = 'p' AS is_primary
             FROM pg_catalog.pg_constraint con
             JOIN pg_catalog.pg_class c ON c.oid = con.conrelid
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
             JOIN LATERAL unnest(con.conkey) WITH ORDINALITY AS u(attnum, ord) ON true
             JOIN pg_catalog.pg_attribute a ON a.attrelid = con.conrelid AND a.attnum = u.attnum
             WHERE con.contype IN ('u', 'p')
               AND n.nspname = $1
               AND c.relname = $2
             GROUP BY con.conname, con.contype
             ORDER BY con.contype DESC, con.conname",
            &[&schema, &table],
        )
        .await
        .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;

    Ok(rows
        .iter()
        .map(|r| UniqueConstraintInfo {
            constraint_name: r.get("constraint_name"),
            columns: r.get::<_, Vec<String>>("columns"),
            is_primary: r.get("is_primary"),
        })
        .collect())
}

pub async fn get_create_table_sql(
    pool: &Pool,
    schema: &str,
    table: &str,
) -> Result<String, SakiError> {
    let client = pool
        .get()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;

    let mut ddl = String::new();

    // 1. Columns
    let col_rows = client
        .query(
            "SELECT a.attname AS column_name,
                    pg_catalog.format_type(a.atttypid, a.atttypmod) AS data_type,
                    a.attnotnull AS not_null,
                    pg_catalog.pg_get_expr(d.adbin, d.adrelid) AS default_value,
                    col_description(c.oid, a.attnum) AS column_comment
             FROM pg_catalog.pg_attribute a
             JOIN pg_catalog.pg_class c ON c.oid = a.attrelid
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
             LEFT JOIN pg_catalog.pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum
             WHERE n.nspname = $1
               AND c.relname = $2
               AND a.attnum > 0
               AND NOT a.attisdropped
             ORDER BY a.attnum",
            &[&schema, &table],
        )
        .await
        .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;

    if col_rows.is_empty() {
        return Err(SakiError::QueryFailed(format!(
            "Table \"{}\".\"{}\" not found",
            schema, table
        )));
    }

    let _ = writeln!(ddl, "CREATE TABLE {}.{} (", quote_ident(schema), quote_ident(table));

    let mut col_defs: Vec<String> = Vec::new();
    for row in &col_rows {
        let col_name: String = row.get("column_name");
        let data_type: String = row.get("data_type");
        let not_null: bool = row.get("not_null");
        let default_value: Option<String> = row.get("default_value");

        let mut col = format!("    {} {}", quote_ident(&col_name), data_type);
        if let Some(ref def) = default_value {
            let _ = write!(col, " DEFAULT {def}");
        }
        if not_null {
            col.push_str(" NOT NULL");
        }
        col_defs.push(col);
    }

    // 2-5. Run constraints, partition, indexes, and comments queries concurrently
    // (tokio-postgres pipelines these on the same connection)
    let params: &[&(dyn tokio_postgres::types::ToSql + Sync)] = &[&schema, &table];
    let (constraint_rows, part_rows, idx_rows, comment_rows) = tokio::try_join!(
        client.query(
            "SELECT con.conname AS constraint_name,
                    con.contype AS constraint_type,
                    pg_catalog.pg_get_constraintdef(con.oid) AS constraint_def
             FROM pg_catalog.pg_constraint con
             JOIN pg_catalog.pg_class c ON c.oid = con.conrelid
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
             WHERE n.nspname = $1
               AND c.relname = $2
             ORDER BY
               CASE con.contype WHEN 'p' THEN 0 WHEN 'u' THEN 1 WHEN 'c' THEN 2 WHEN 'f' THEN 3 ELSE 4 END,
               con.conname",
            params,
        ),
        client.query(
            "SELECT pg_catalog.pg_get_partkeydef(c.oid) AS partition_key
             FROM pg_catalog.pg_class c
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
             WHERE n.nspname = $1
               AND c.relname = $2
               AND c.relkind = 'p'",
            params,
        ),
        client.query(
            "SELECT pg_catalog.pg_get_indexdef(i.indexrelid) AS index_def
             FROM pg_catalog.pg_index i
             JOIN pg_catalog.pg_class c ON c.oid = i.indrelid
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
             LEFT JOIN pg_catalog.pg_constraint con ON con.conindid = i.indexrelid
             WHERE n.nspname = $1
               AND c.relname = $2
               AND con.oid IS NULL
             ORDER BY i.indexrelid",
            params,
        ),
        client.query(
            "SELECT obj_description(c.oid) AS table_comment
             FROM pg_catalog.pg_class c
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
             WHERE n.nspname = $1 AND c.relname = $2",
            params,
        ),
    )
    .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;

    let mut constraint_defs: Vec<String> = Vec::with_capacity(constraint_rows.len());
    for row in &constraint_rows {
        let name: String = row.get("constraint_name");
        let def: String = row.get("constraint_def");
        let mut s = String::with_capacity(16 + name.len() + def.len());
        let _ = write!(s, "    CONSTRAINT {} {}", quote_ident(&name), def);
        constraint_defs.push(s);
    }

    let all_defs: Vec<String> = col_defs
        .into_iter()
        .chain(constraint_defs.into_iter())
        .collect();
    ddl.push_str(&all_defs.join(",\n"));
    ddl.push('\n');
    ddl.push_str(")");

    if let Some(part_row) = part_rows.first() {
        let partition_key: String =
            part_row.get::<_, Option<String>>("partition_key").unwrap_or_default();
        if !partition_key.is_empty() {
            let _ = write!(ddl, "\nPARTITION BY {partition_key}");
        }
    }

    ddl.push_str(";\n");

    for row in &idx_rows {
        let index_def: String = row.get("index_def");
        let _ = write!(ddl, "\n{index_def};\n");
    }

    if let Some(row) = comment_rows.first() {
        let comment: Option<String> = row.get("table_comment");
        if let Some(c) = comment {
            let _ = write!(
                ddl,
                "\nCOMMENT ON TABLE {}.{} IS '{}';\n",
                quote_ident(schema),
                quote_ident(table),
                c.replace('\'', "''")
            );
        }
    }

    // Column comments
    for row in &col_rows {
        let col_name: String = row.get("column_name");
        let comment: Option<String> = row.get("column_comment");
        if let Some(c) = comment {
            let _ = writeln!(
                ddl,
                "COMMENT ON COLUMN {}.{}.{} IS '{}';",
                quote_ident(schema),
                quote_ident(table),
                quote_ident(&col_name),
                c.replace('\'', "''")
            );
        }
    }

    Ok(ddl)
}

pub async fn get_partition_info(
    pool: &Pool,
    schema: &str,
    table: &str,
) -> Result<Option<PartitionInfo>, SakiError> {
    let client = pool
        .get()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;

    // Check if table is partitioned
    let rows = client
        .query(
            "SELECT pt.partstrat::text AS strategy,
                    pg_catalog.pg_get_partkeydef(c.oid) AS partition_key
             FROM pg_catalog.pg_class c
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
             JOIN pg_catalog.pg_partitioned_table pt ON pt.partrelid = c.oid
             WHERE n.nspname = $1
               AND c.relname = $2",
            &[&schema, &table],
        )
        .await
        .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;

    let row = match rows.first() {
        Some(r) => r,
        None => return Ok(None),
    };

    let strategy_char: String = row.get("strategy");
    let strategy = match strategy_char.as_str() {
        "r" => "RANGE",
        "l" => "LIST",
        "h" => "HASH",
        _ => "UNKNOWN",
    }
    .to_string();

    let partition_key: String = row.get::<_, Option<String>>("partition_key").unwrap_or_default();

    // Get partitions
    let part_rows = client
        .query(
            "SELECT child.relname AS partition_name,
                    pg_catalog.pg_get_expr(child.relpartbound, child.oid) AS expression,
                    child.reltuples::bigint AS row_estimate
             FROM pg_catalog.pg_inherits i
             JOIN pg_catalog.pg_class parent ON parent.oid = i.inhparent
             JOIN pg_catalog.pg_namespace pn ON pn.oid = parent.relnamespace
             JOIN pg_catalog.pg_class child ON child.oid = i.inhrelid
             WHERE pn.nspname = $1
               AND parent.relname = $2
             ORDER BY child.relname",
            &[&schema, &table],
        )
        .await
        .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;

    let partitions = part_rows
        .iter()
        .map(|r| PartitionDetail {
            name: r.get("partition_name"),
            expression: r.get::<_, Option<String>>("expression").unwrap_or_default(),
            row_count_estimate: r.get("row_estimate"),
        })
        .collect();

    Ok(Some(PartitionInfo {
        strategy,
        partition_key,
        partitions,
    }))
}

pub async fn list_schema_foreign_keys(
    pool: &Pool,
    schema: &str,
) -> Result<HashMap<String, Vec<ForeignKeyInfo>>, SakiError> {
    let client = pool
        .get()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;

    let rows = client
        .query(
            "SELECT
                 con.conname AS constraint_name,
                 c.relname AS table_name,
                 array_agg(a.attname ORDER BY u.attposition) AS columns,
                 fn.nspname AS foreign_table_schema,
                 fc.relname AS foreign_table_name,
                 array_agg(fa.attname ORDER BY u.attposition) AS foreign_columns,
                 CASE con.confupdtype
                     WHEN 'a' THEN 'NO ACTION'
                     WHEN 'r' THEN 'RESTRICT'
                     WHEN 'c' THEN 'CASCADE'
                     WHEN 'n' THEN 'SET NULL'
                     WHEN 'd' THEN 'SET DEFAULT'
                     ELSE 'NO ACTION'
                 END AS on_update,
                 CASE con.confdeltype
                     WHEN 'a' THEN 'NO ACTION'
                     WHEN 'r' THEN 'RESTRICT'
                     WHEN 'c' THEN 'CASCADE'
                     WHEN 'n' THEN 'SET NULL'
                     WHEN 'd' THEN 'SET DEFAULT'
                     ELSE 'NO ACTION'
                 END AS on_delete
             FROM pg_catalog.pg_constraint con
             JOIN pg_catalog.pg_class c ON c.oid = con.conrelid
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
             JOIN pg_catalog.pg_class fc ON fc.oid = con.confrelid
             JOIN pg_catalog.pg_namespace fn ON fn.oid = fc.relnamespace
             JOIN LATERAL unnest(con.conkey, con.confkey) WITH ORDINALITY AS u(local_attnum, foreign_attnum, attposition) ON true
             JOIN pg_catalog.pg_attribute a ON a.attrelid = con.conrelid AND a.attnum = u.local_attnum
             JOIN pg_catalog.pg_attribute fa ON fa.attrelid = con.confrelid AND fa.attnum = u.foreign_attnum
             WHERE con.contype = 'f'
               AND n.nspname = $1
             GROUP BY con.conname, c.relname, fn.nspname, fc.relname, con.confupdtype, con.confdeltype
             ORDER BY c.relname, con.conname",
            &[&schema],
        )
        .await
        .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;

    let mut result: HashMap<String, Vec<ForeignKeyInfo>> = HashMap::new();
    for r in &rows {
        let table_name: String = r.get("table_name");
        let fk = ForeignKeyInfo {
            constraint_name: r.get("constraint_name"),
            columns: r.get::<_, Vec<String>>("columns"),
            foreign_table_schema: r.get("foreign_table_schema"),
            foreign_table_name: r.get("foreign_table_name"),
            foreign_columns: r.get::<_, Vec<String>>("foreign_columns"),
            on_update: r.get("on_update"),
            on_delete: r.get("on_delete"),
        };
        result.entry(table_name).or_default().push(fk);
    }
    Ok(result)
}

pub async fn list_schema_columns(
    pool: &Pool,
    schema: &str,
) -> Result<HashMap<String, Vec<ColumnInfo>>, SakiError> {
    let client = pool
        .get()
        .await
        .map_err(|e| SakiError::QueryFailed(format_pool_error(&e)))?;

    let rows = client
        .query(
            "SELECT c.table_name, c.column_name, c.data_type, c.is_nullable, c.column_default,
                    COALESCE(pk.is_pk, false) AS is_pk
             FROM information_schema.columns c
             LEFT JOIN (
                 SELECT kcu.table_name, kcu.column_name, true AS is_pk
                 FROM information_schema.table_constraints tc
                 JOIN information_schema.key_column_usage kcu
                     ON tc.constraint_name = kcu.constraint_name
                     AND tc.table_schema = kcu.table_schema
                 WHERE tc.constraint_type = 'PRIMARY KEY'
                     AND tc.table_schema = $1
             ) pk ON pk.table_name = c.table_name AND pk.column_name = c.column_name
             WHERE c.table_schema = $1
             ORDER BY c.table_name, c.ordinal_position",
            &[&schema],
        )
        .await
        .map_err(|e| SakiError::QueryFailed(format_pg_error(&e)))?;

    let mut result: HashMap<String, Vec<ColumnInfo>> = HashMap::new();
    for r in &rows {
        let table_name: String = r.get("table_name");
        let col = ColumnInfo {
            name: r.get("column_name"),
            data_type: r.get("data_type"),
            is_nullable: r.get::<_, String>("is_nullable") == "YES",
            is_primary_key: r.get("is_pk"),
            default_value: r.get("column_default"),
        };
        result.entry(table_name).or_default().push(col);
    }
    Ok(result)
}

pub async fn get_erd_data(pool: &Pool, schema: &str) -> Result<ErdData, SakiError> {
    debug!(schema, "loading ERD data");
    let (tables, columns, foreign_keys) = tokio::try_join!(
        list_tables(pool, schema),
        list_schema_columns(pool, schema),
        list_schema_foreign_keys(pool, schema),
    )?;

    Ok(ErdData {
        tables,
        columns,
        foreign_keys,
    })
}
