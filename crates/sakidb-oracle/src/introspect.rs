use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use oracle::Connection as OracleConnection;
use oracle::sql_type::{ToSql, Clob};
use dashmap::DashMap;
use sakidb_core::{
    error::{Result, SakiError},
    types::{
        ConnectionId, DatabaseInfo, SchemaInfo, TableInfo, ColumnInfo, ViewInfo,
        MaterializedViewInfo, FunctionInfo, SequenceInfo, IndexInfo, TriggerInfo,
        ForeignKeyInfo, CheckConstraintInfo, UniqueConstraintInfo, PartitionInfo,
        PartitionDetail, ErdData, CompletionTable, CompletionBundle, CompletionColumn,
    },
};
use tracing::debug;

pub struct OracleIntrospector {
    pub(crate) connections: Arc<DashMap<ConnectionId, Arc<RwLock<OracleConnection>>>>,
}

impl OracleIntrospector {
    pub fn new(connections: Arc<DashMap<ConnectionId, Arc<RwLock<OracleConnection>>>>) -> Self {
        Self { connections }
    }

    pub(crate) fn get_connection(&self, conn_id: &ConnectionId) -> Result<Arc<RwLock<OracleConnection>>> {
        self.connections
            .get(conn_id)
            .map(|entry| entry.clone())
            .ok_or_else(|| SakiError::ConnectionNotFound(conn_id.0.to_string()))
    }

    async fn execute_query(conn: Arc<RwLock<OracleConnection>>, query: String, params: Vec<String>) -> Result<Vec<oracle::Row>> {
        // [Fix: M1] Use bind parameters for all introspection queries to prevent SQL injection
        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_read();
            // Convert String vec to &dyn ToSql slice
            let bind_params: Vec<&dyn ToSql> = params.iter().map(|s| s as &dyn ToSql).collect();
            let result_set = conn.query(&query, &bind_params)
                .map_err(|e| SakiError::QueryFailed(format!("Introspection query failed: {}", e)))?;
            let mut rows = Vec::new();
            for row_result in result_set {
                let row = row_result
                    .map_err(|e| SakiError::QueryFailed(format!("Row fetch error: {}", e)))?;
                rows.push(row);
            }
            Ok::<Vec<oracle::Row>, SakiError>(rows)
        })
        .await
        .map_err(|e| SakiError::QueryFailed(format!("Introspection task failed: {}", e)))?
    }

    pub async fn list_databases(&self, conn_id: &ConnectionId) -> Result<Vec<DatabaseInfo>> {
        let conn = self.get_connection(conn_id)?;
        let rows = Self::execute_query(
            conn,
            "SELECT name FROM v$database WHERE name IS NOT NULL".to_string(),
            vec![],
        )
        .await?;

        let mut databases = Vec::new();
        for row in rows {
            let name: String = row.get(0)
                .map_err(|e| SakiError::QueryFailed(format!("Failed to get db name: {}", e)))?;
            databases.push(DatabaseInfo { name, is_template: false });
        }
        Ok(databases)
    }

    pub async fn list_schemas(&self, conn_id: &ConnectionId) -> Result<Vec<SchemaInfo>> {
        let conn = self.get_connection(conn_id)?;
        // [Fix: Minor 1] Expanded system schema exclusions for Oracle 19c/21c
        let rows = Self::execute_query(conn, "
            SELECT username
            FROM all_users
            WHERE username NOT IN (
                'SYS','SYSTEM','DBSNMP','OUTLN','FLOWS_FILES','MDSYS','ORDSYS',
                'EXFSYS','WMSYS','APPQOSSYS','APEX_030200','OWBSYS','CTXSYS',
                'ANONYMOUS','SYSMAN','XDB','XS$NULL','SI_INFORMTN_SCHEMA',
                'OLAPSYS','ORDDATA','DIP','ORDPLUGINS','MDDATA',
                'AUDSYS','DBSFWUSER','GGSYS','REMOTE_SCHEDULER_AGENT','GSMADMIN_INTERNAL',
                'DVF','DVSYS','LBACSYS','OJVMSYS','WJPYS','WKPYS'
            )
            ORDER BY username
        ".to_string(), vec![]).await?;

        let mut schemas = Vec::new();
        for row in rows {
            let name: String = row.get(0)
                .map_err(|e| SakiError::QueryFailed(format!("Failed to get schema name: {}", e)))?;
            schemas.push(SchemaInfo { name });
        }
        Ok(schemas)
    }

    pub async fn list_tables(&self, conn_id: &ConnectionId, schema: &str) -> Result<Vec<TableInfo>> {
        let conn = self.get_connection(conn_id)?;
        let query = "SELECT table_name, num_rows, blocks
             FROM all_tables
             WHERE owner = :1
             AND table_name NOT LIKE 'BIN$%'
             ORDER BY table_name".to_string();
        let rows = Self::execute_query(conn, query, vec![schema.to_uppercase()]).await?;

        let mut tables = Vec::new();
        for row in rows {
            let name: String = row.get(0)
                .map_err(|e| SakiError::QueryFailed(format!("Failed to get table name: {}", e)))?;
            let row_count: Option<f64> = row.get(1).ok().flatten();
            let blocks: Option<f64> = row.get(2).ok().flatten();
            tables.push(TableInfo {
                name,
                row_count_estimate: row_count.map(|r| r as i64),
                size_bytes: blocks.map(|b| (b * 8192.0) as i64),
                is_partition: false,
                parent_table: None,
            });
        }
        Ok(tables)
    }

    pub async fn list_columns(&self, conn_id: &ConnectionId, schema: &str, table: &str) -> Result<Vec<ColumnInfo>> {
        let conn = self.get_connection(conn_id)?;
        let query = "SELECT column_name, data_type, nullable, data_default
             FROM all_tab_columns
             WHERE owner = :1 AND table_name = :2
             ORDER BY column_id".to_string();
        let rows = Self::execute_query(conn.clone(), query, vec![schema.to_uppercase(), table.to_uppercase()]).await?;

        let mut columns = Vec::new();
        for row in rows {
            let name: String = row.get(0)
                .map_err(|e| SakiError::QueryFailed(format!("Failed to get column name: {}", e)))?;
            let data_type: String = row.get(1)
                .map_err(|e| SakiError::QueryFailed(format!("Failed to get data_type: {}", e)))?;
            let nullable: String = row.get(2)
                .map_err(|e| SakiError::QueryFailed(format!("Failed to get nullable: {}", e)))?;
            let default_value: Option<String> = row.get(3).ok().flatten();
            columns.push(ColumnInfo {
                name,
                data_type,
                is_nullable: nullable == "Y",
                is_primary_key: false,
                default_value,
            });
        }

        // Mark primary key columns
        let pk_query = "SELECT acc.column_name
             FROM all_cons_columns acc
             JOIN all_constraints ac ON acc.constraint_name = ac.constraint_name AND acc.owner = ac.owner
             WHERE ac.owner = :1 AND ac.table_name = :2 AND ac.constraint_type = 'P'".to_string();
        let pk_rows = Self::execute_query(conn, pk_query, vec![schema.to_uppercase(), table.to_uppercase()]).await?;
        for row in pk_rows {
            let pk_col: String = row.get(0)
                .map_err(|e| SakiError::QueryFailed(format!("Failed to get pk col: {}", e)))?;
            if let Some(col) = columns.iter_mut().find(|c| c.name.to_uppercase() == pk_col.to_uppercase()) {
                col.is_primary_key = true;
            }
        }

        Ok(columns)
    }

    pub async fn list_views(&self, conn_id: &ConnectionId, schema: &str) -> Result<Vec<ViewInfo>> {
        let conn = self.get_connection(conn_id)?;
        let query = "SELECT view_name, read_only FROM all_views WHERE owner = :1 ORDER BY view_name".to_string();
        let rows = Self::execute_query(conn, query, vec![schema.to_uppercase()]).await?;
        let mut views = Vec::new();
        for row in rows {
            let name: String = row.get(0)
                .map_err(|e| SakiError::QueryFailed(format!("Failed to get view name: {}", e)))?;
            let read_only: String = row.get(1).unwrap_or_else(|_| "N".to_string());
            views.push(ViewInfo { name, is_updatable: read_only == "N" });
        }
        Ok(views)
    }

    pub async fn list_materialized_views(&self, conn_id: &ConnectionId, schema: &str) -> Result<Vec<MaterializedViewInfo>> {
        let conn = self.get_connection(conn_id)?;
        let query = "SELECT mview_name, num_rows, last_refresh_date FROM all_mviews WHERE owner = :1 ORDER BY mview_name".to_string();
        let rows = Self::execute_query(conn, query, vec![schema.to_uppercase()]).await?;
        let mut mviews = Vec::new();
        for row in rows {
            let name: String = row.get(0)
                .map_err(|e| SakiError::QueryFailed(format!("Failed to get mview name: {}", e)))?;
            let row_count: Option<f64> = row.get(1).ok().flatten();
            // last_refresh_date — get as string or null
            let has_refresh: bool = row.get::<_, Option<String>>(2).unwrap_or(None).is_some();
            mviews.push(MaterializedViewInfo {
                name,
                row_count_estimate: row_count.map(|r| r as i64),
                is_populated: has_refresh,
            });
        }
        Ok(mviews)
    }

    pub async fn list_functions(&self, conn_id: &ConnectionId, schema: &str) -> Result<Vec<FunctionInfo>> {
        let conn = self.get_connection(conn_id)?;
        let query = "SELECT object_name, object_type FROM all_procedures
             WHERE owner = :1 AND object_type IN ('FUNCTION', 'PROCEDURE', 'PACKAGE')
             ORDER BY object_name".to_string();
        let rows = Self::execute_query(conn, query, vec![schema.to_uppercase()]).await?;
        let mut functions = Vec::new();
        for row in rows {
            let name: String = row.get(0)
                .map_err(|e| SakiError::QueryFailed(format!("Failed to get function name: {}", e)))?;
            let kind: String = row.get(1)
                .map_err(|e| SakiError::QueryFailed(format!("Failed to get function kind: {}", e)))?;
            functions.push(FunctionInfo {
                name,
                kind,
                return_type: "UNKNOWN".to_string(),
                argument_types: String::new(),
                language: "PL/SQL".to_string(),
            });
        }
        Ok(functions)
    }

    pub async fn list_sequences(&self, conn_id: &ConnectionId, schema: &str) -> Result<Vec<SequenceInfo>> {
        let conn = self.get_connection(conn_id)?;
        let query = "SELECT sequence_name, last_number FROM all_sequences WHERE sequence_owner = :1 ORDER BY sequence_name".to_string();
        let rows = Self::execute_query(conn, query, vec![schema.to_uppercase()]).await?;
        let mut sequences = Vec::new();
        for row in rows {
            let name: String = row.get(0)
                .map_err(|e| SakiError::QueryFailed(format!("Failed to get sequence name: {}", e)))?;
            let last_value: Option<f64> = row.get(1).ok().flatten();
            sequences.push(SequenceInfo {
                name,
                data_type: "NUMBER".to_string(),
                last_value: last_value.map(|v| v as i64),
            });
        }
        Ok(sequences)
    }

    pub async fn list_indexes(&self, conn_id: &ConnectionId, schema: &str) -> Result<Vec<IndexInfo>> {
        let conn = self.get_connection(conn_id)?;
        let query = "SELECT aic.index_name, aic.table_name, aic.column_name, ai.uniqueness, ai.index_type
             FROM all_ind_columns aic
             JOIN all_indexes ai ON aic.index_name = ai.index_name AND aic.table_owner = ai.table_owner
             WHERE ai.table_owner = :1
             ORDER BY aic.index_name, aic.column_position".to_string();
        let rows = Self::execute_query(conn.clone(), query, vec![schema.to_uppercase()]).await?;

        let mut index_map: HashMap<String, IndexInfo> = HashMap::new();
        for row in rows {
            let index_name: String = row.get(0)
                .map_err(|e| SakiError::QueryFailed(format!("Failed to get index name: {}", e)))?;
            let table_name: String = row.get(1)
                .map_err(|e| SakiError::QueryFailed(format!("Failed to get table name: {}", e)))?;
            let column_name: String = row.get(2)
                .map_err(|e| SakiError::QueryFailed(format!("Failed to get column name: {}", e)))?;
            let uniqueness: String = row.get(3)
                .map_err(|e| SakiError::QueryFailed(format!("Failed to get uniqueness: {}", e)))?;
            let index_type: String = row.get(4)
                .map_err(|e| SakiError::QueryFailed(format!("Failed to get index type: {}", e)))?;

            let entry = index_map.entry(index_name.clone()).or_insert(IndexInfo {
                name: index_name,
                table_name,
                columns: String::new(),
                is_unique: uniqueness == "UNIQUE",
                is_primary: false,
                index_type,
            });
            if !entry.columns.is_empty() {
                entry.columns.push_str(", ");
            }
            entry.columns.push_str(&column_name);
        }

        // Mark primary key indexes
        let pk_query = "SELECT index_name FROM all_constraints WHERE owner = :1 AND constraint_type = 'P'".to_string();
        let pk_rows = Self::execute_query(conn, pk_query, vec![schema.to_uppercase()]).await?;
        for row in pk_rows {
            let pk_idx: String = row.get(0).unwrap_or_default();
            if let Some(idx) = index_map.get_mut(&pk_idx) {
                idx.is_primary = true;
            }
        }

        Ok(index_map.into_values().collect())
    }

    pub async fn list_triggers(&self, conn_id: &ConnectionId, schema: &str, table: &str) -> Result<Vec<TriggerInfo>> {
        let conn = self.get_connection(conn_id)?;
        let query = "SELECT trigger_name, table_name, triggering_event, trigger_type, when_clause, trigger_body, status
             FROM all_triggers
             WHERE owner = :1 AND table_name = :2
             ORDER BY trigger_name".to_string();
        let rows = Self::execute_query(conn, query, vec![schema.to_uppercase(), table.to_uppercase()]).await?;
        let mut triggers = Vec::new();
        for row in rows {
            let name: String = row.get(0)
                .map_err(|e| SakiError::QueryFailed(format!("Failed to get trigger name: {}", e)))?;
            let table_name: String = row.get(1)
                .map_err(|e| SakiError::QueryFailed(format!("Failed to get trigger table: {}", e)))?;
            let event: String = row.get(2)
                .map_err(|e| SakiError::QueryFailed(format!("Failed to get trigger event: {}", e)))?;
            let tt: String = row.get(3)
                .map_err(|e| SakiError::QueryFailed(format!("Failed to get trigger type: {}", e)))?;
            let condition: Option<String> = row.get(4).ok().flatten();
            let function_name: String = row.get::<_, String>(5).unwrap_or_default();
            let status: String = row.get(6)
                .map_err(|e| SakiError::QueryFailed(format!("Failed to get trigger status: {}", e)))?;

            // [Fix: M6] Parse trigger_type (e.g., "BEFORE EACH ROW", "AFTER STATEMENT")
            // Instead of hardcoding 'ROW', we extract timing and for_each correctly.
            let (timing, for_each) = if tt.contains("EACH ROW") {
                (tt.replace(" EACH ROW", ""), "ROW".to_string())
            } else {
                (tt, "STATEMENT".to_string())
            };

            triggers.push(TriggerInfo {
                name,
                table_name,
                event,
                timing,
                for_each,
                function_name,
                function_schema: schema.to_string(),
                condition,
                is_enabled: status == "ENABLED",
            });
        }
        Ok(triggers)
    }

    pub async fn list_foreign_keys(&self, conn_id: &ConnectionId, schema: &str, table: &str) -> Result<Vec<ForeignKeyInfo>> {
        let conn = self.get_connection(conn_id)?;
        let query = "SELECT
                a.constraint_name,
                LISTAGG(a.column_name, ', ') WITHIN GROUP (ORDER BY a.position) AS src_cols,
                r.owner AS fk_schema,
                r.table_name AS fk_table,
                LISTAGG(cr.column_name, ', ') WITHIN GROUP (ORDER BY cr.position) AS fk_cols,
                c.delete_rule,
                'NO ACTION' AS update_rule
             FROM all_cons_columns a
             JOIN all_constraints c ON a.constraint_name = c.constraint_name AND a.owner = c.owner
             JOIN all_constraints r ON c.r_constraint_name = r.constraint_name
             JOIN all_cons_columns cr ON r.constraint_name = cr.constraint_name
             WHERE c.owner = :1 AND c.table_name = :2 AND c.constraint_type = 'R'
             GROUP BY a.constraint_name, r.owner, r.table_name, c.delete_rule".to_string();
        let rows = Self::execute_query(conn, query, vec![schema.to_uppercase(), table.to_uppercase()]).await?;
        let mut fks = Vec::new();
        for row in rows {
            let constraint_name: String = row.get(0)
                .map_err(|e| SakiError::QueryFailed(format!("Failed to get fk name: {}", e)))?;
            let columns: String = row.get(1).unwrap_or_default();
            let foreign_table_schema: String = row.get(2).unwrap_or_default();
            let foreign_table_name: String = row.get(3).unwrap_or_default();
            let foreign_columns: String = row.get(4).unwrap_or_default();
            let on_delete: String = row.get(5).unwrap_or_else(|_| "NO ACTION".to_string());
            let on_update: String = row.get(6).unwrap_or_else(|_| "NO ACTION".to_string());
            fks.push(ForeignKeyInfo {
                constraint_name,
                columns: columns.split(", ").map(|s| s.to_string()).collect(),
                foreign_table_schema,
                foreign_table_name,
                foreign_columns: foreign_columns.split(", ").map(|s| s.to_string()).collect(),
                on_delete,
                on_update,
            });
        }
        Ok(fks)
    }

    pub async fn list_check_constraints(&self, conn_id: &ConnectionId, schema: &str, table: &str) -> Result<Vec<CheckConstraintInfo>> {
        let conn = self.get_connection(conn_id)?;
        let query = "SELECT constraint_name, search_condition
             FROM all_constraints
             WHERE owner = :1 AND table_name = :2 AND constraint_type = 'C'
             ORDER BY constraint_name".to_string();
        let rows = Self::execute_query(conn, query, vec![schema.to_uppercase(), table.to_uppercase()]).await?;
        let mut constraints = Vec::new();
        for row in rows {
            let constraint_name: String = row.get(0)
                .map_err(|e| SakiError::QueryFailed(format!("Failed to get constraint name: {}", e)))?;
            let check_clause: String = row.get(1).unwrap_or_default();
            constraints.push(CheckConstraintInfo { constraint_name, check_clause });
        }
        Ok(constraints)
    }

    pub async fn list_unique_constraints(&self, conn_id: &ConnectionId, schema: &str, table: &str) -> Result<Vec<UniqueConstraintInfo>> {
        let conn = self.get_connection(conn_id)?;
        let query = "SELECT
                a.constraint_name,
                LISTAGG(a.column_name, ', ') WITHIN GROUP (ORDER BY a.position) AS cols
             FROM all_cons_columns a
             JOIN all_constraints c ON a.constraint_name = c.constraint_name AND a.owner = c.owner
             WHERE c.owner = :1 AND c.table_name = :2 AND c.constraint_type = 'U'
             GROUP BY a.constraint_name".to_string();
        let rows = Self::execute_query(conn, query, vec![schema.to_uppercase(), table.to_uppercase()]).await?;
        let mut constraints = Vec::new();
        for row in rows {
            let constraint_name: String = row.get(0)
                .map_err(|e| SakiError::QueryFailed(format!("Failed to get unique constraint name: {}", e)))?;
            let columns: String = row.get(1).unwrap_or_default();
            constraints.push(UniqueConstraintInfo {
                constraint_name,
                columns: columns.split(", ").map(|s| s.to_string()).collect(),
                is_primary: false,
            });
        }
        Ok(constraints)
    }

    pub async fn get_partition_info(&self, conn_id: &ConnectionId, schema: &str, table: &str) -> Result<Option<PartitionInfo>> {
        let conn = self.get_connection(conn_id)?;
        let query = "SELECT partitioning_type, partition_count FROM all_part_tables WHERE owner = :1 AND table_name = :2".to_string();
        let rows = Self::execute_query(conn.clone(), query, vec![schema.to_uppercase(), table.to_uppercase()]).await?;
        if rows.is_empty() {
            return Ok(None);
        }

        let partitioning_type: String = rows[0].get(0).unwrap_or_else(|_| "RANGE".to_string());

        let detail_query = "SELECT partition_name, high_value FROM all_tab_partitions
             WHERE table_owner = :1 AND table_name = :2 ORDER BY partition_position".to_string();
        let detail_rows = Self::execute_query(conn, detail_query, vec![schema.to_uppercase(), table.to_uppercase()]).await?;
        let mut partitions = Vec::new();
        for row in detail_rows {
            let name: String = row.get(0).unwrap_or_default();
            let expression: String = row.get(1).unwrap_or_default();
            partitions.push(PartitionDetail {
                name,
                expression,
                row_count_estimate: None,
            });
        }

        Ok(Some(PartitionInfo {
            strategy: partitioning_type,
            partition_key: String::new(),
            partitions,
        }))
    }

    pub async fn get_create_table_sql(&self, conn_id: &ConnectionId, schema: &str, table: &str) -> Result<String> {
        let conn = self.get_connection(conn_id)?;
        // [Fix: M5] Use bind parameters for DBMS_METADATA.GET_DDL
        let query = "SELECT dbms_metadata.get_ddl('TABLE', :1, :2) FROM dual".to_string();
        let rows = Self::execute_query(conn, query, vec![table.to_uppercase(), schema.to_uppercase()]).await?;
        if rows.is_empty() {
            return Err(SakiError::QueryFailed("Table not found".to_string()));
        }
        let ddl: String = self.row_to_string_or_clob(&rows[0], 0)?;
        Ok(ddl)
    }

    pub async fn get_erd_data(&self, conn_id: &ConnectionId, schema: &str) -> Result<ErdData> {
        let conn = self.get_connection(conn_id)?;
        
        let tables = self.list_tables(conn_id, schema).await?;
        
        // Batch query for all columns in the schema
        let col_query = "SELECT table_name, column_name, data_type, nullable, data_default
             FROM all_tab_columns
             WHERE owner = :1
             ORDER BY table_name, column_id".to_string();
        let col_rows = Self::execute_query(conn.clone(), col_query, vec![schema.to_uppercase()]).await?;
        
        // Batch query for all primary keys in the schema
        let pk_query = "SELECT acc.table_name, acc.column_name
             FROM all_cons_columns acc
             JOIN all_constraints ac ON acc.constraint_name = ac.constraint_name AND acc.owner = ac.owner
             WHERE ac.owner = :1 AND ac.constraint_type = 'P'".to_string();
        let pk_rows = Self::execute_query(conn.clone(), pk_query, vec![schema.to_uppercase()]).await?;
        
        let mut pk_map: HashMap<String, Vec<String>> = HashMap::new();
        for row in pk_rows {
            let table_name: String = row.get(0).unwrap_or_default();
            let col_name: String = row.get(1).unwrap_or_default();
            pk_map.entry(table_name).or_default().push(col_name);
        }

        let mut columns = HashMap::new();
        for row in col_rows {
            let table_name: String = row.get(0).unwrap_or_default();
            let name: String = row.get(1).unwrap_or_default();
            let data_type: String = row.get(2).unwrap_or_default();
            let nullable: String = row.get(3).unwrap_or_default();
            let default_value: Option<String> = row.get(4).ok().flatten();
            
            let is_primary_key = pk_map.get(&table_name)
                .map(|pks| pks.contains(&name))
                .unwrap_or(false);

            columns.entry(table_name).or_insert_with(Vec::new).push(ColumnInfo {
                name,
                data_type,
                is_nullable: nullable == "Y",
                is_primary_key,
                default_value,
            });
        }

        // Batch query for all foreign keys in the schema
        let fk_query = "SELECT
                a.table_name,
                a.constraint_name,
                LISTAGG(a.column_name, ', ') WITHIN GROUP (ORDER BY a.position) AS src_cols,
                r.owner AS fk_schema,
                r.table_name AS fk_table,
                LISTAGG(cr.column_name, ', ') WITHIN GROUP (ORDER BY cr.position) AS fk_cols,
                c.delete_rule
             FROM all_cons_columns a
             JOIN all_constraints c ON a.constraint_name = c.constraint_name AND a.owner = c.owner
             JOIN all_constraints r ON c.r_constraint_name = r.constraint_name
             JOIN all_cons_columns cr ON r.constraint_name = cr.constraint_name
             WHERE c.owner = :1 AND c.constraint_type = 'R'
             GROUP BY a.table_name, a.constraint_name, r.owner, r.table_name, c.delete_rule".to_string();
        let fk_rows = Self::execute_query(conn, fk_query, vec![schema.to_uppercase()]).await?;
        
        let mut foreign_keys = HashMap::new();
        for row in fk_rows {
            let table_name: String = row.get(0).unwrap_or_default();
            let constraint_name: String = row.get(1).unwrap_or_default();
            let src_cols: String = row.get(2).unwrap_or_default();
            let fk_schema: String = row.get(3).unwrap_or_default();
            let fk_table: String = row.get(4).unwrap_or_default();
            let fk_cols: String = row.get(5).unwrap_or_default();
            let on_delete: String = row.get(6).unwrap_or_else(|_| "NO ACTION".to_string());
            
            foreign_keys.entry(table_name).or_insert_with(Vec::new).push(ForeignKeyInfo {
                constraint_name,
                columns: src_cols.split(", ").map(|s| s.to_string()).collect(),
                foreign_table_schema: fk_schema,
                foreign_table_name: fk_table,
                foreign_columns: fk_cols.split(", ").map(|s| s.to_string()).collect(),
                on_delete,
                on_update: "NO ACTION".to_string(),
            });
        }

        Ok(ErdData { tables, columns, foreign_keys })
    }

    pub async fn get_schema_completion_data(&self, conn_id: &ConnectionId, schema: &str) -> Result<HashMap<String, Vec<String>>> {
        let conn = self.get_connection(conn_id)?;
        let query = "SELECT table_name, column_name
             FROM all_tab_columns
             WHERE owner = :1
             ORDER BY table_name, column_id".to_string();
        let rows = Self::execute_query(conn, query, vec![schema.to_uppercase()]).await?;
        
        let mut result: HashMap<String, Vec<String>> = HashMap::new();
        for row in rows {
            let table_name: String = row.get(0).unwrap_or_default();
            let col_name: String = row.get(1).unwrap_or_default();
            result.entry(table_name).or_default().push(col_name);
        }
        Ok(result)
    }

    pub async fn get_completion_bundle(&self, conn_id: &ConnectionId, schema: &str) -> Result<CompletionBundle> {
        let tables = self.list_tables(conn_id, schema).await?;
        let views = self.list_views(conn_id, schema).await?;
        let functions = self.list_functions(conn_id, schema).await?;
        let mut completion_tables: Vec<CompletionTable> = tables
            .into_iter()
            .map(|t| CompletionTable { name: t.name, kind: "table".to_string() })
            .collect();
        for view in views {
            completion_tables.push(CompletionTable { name: view.name, kind: "view".to_string() });
        }
        Ok(CompletionBundle { tables: completion_tables, functions })
    }

    pub async fn get_table_columns_for_completion(&self, conn_id: &ConnectionId, schema: &str, table: &str) -> Result<Vec<CompletionColumn>> {
        let cols = self.list_columns(conn_id, schema, table).await?;
        Ok(cols.into_iter().map(|c| CompletionColumn {
            name: c.name,
            data_type: c.data_type,
            is_primary_key: c.is_primary_key,
            is_nullable: c.is_nullable,
        }).collect())
    }

    // Needed by execute_query in introspect internally - kept as a debug helper
    #[allow(dead_code)]
    fn log_query(query: &str) {
        debug!("Oracle introspect query: {}", query);
    }

    fn row_to_string_or_clob(&self, row: &oracle::Row, idx: usize) -> Result<String> {
        // [Fix: M5] Oracle DBMS_METADATA often returns CLOB which requires special handling
        match row.get::<_, Option<String>>(idx) {
            Ok(Some(s)) => Ok(s),
            Ok(None) => Ok(String::new()),
            Err(_) => {
                // Try as CLOB explicitly if String fails
                match row.get::<_, Option<Clob>>(idx) {
                    Ok(Some(mut clob)) => {
                        let mut s = String::new();
                        use std::io::Read;
                        clob.read_to_string(&mut s).map_err(|e| SakiError::QueryFailed(format!("Failed to read CLOB: {}", e)))?;
                        Ok(s)
                    }
                    _ => Ok(String::new())
                }
            }
        }
    }
}
