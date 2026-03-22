use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;
use oracle::Connection as OracleConnection;
use dashmap::DashMap;
use sakidb_core::{
    error::{Result, SakiError},
    types::{ConnectionId, RestoreOptions, RestoreProgress},
};
use tracing::{info, debug, error};
use crate::sql_split::split_sql_statements;

pub struct OracleRestorer {
    connections: Arc<DashMap<ConnectionId, Arc<RwLock<OracleConnection>>>>,
}

impl OracleRestorer {
    pub fn new(connections: Arc<DashMap<ConnectionId, Arc<RwLock<OracleConnection>>>>) -> Self {
        Self { connections }
    }

    fn get_connection(&self, conn_id: &ConnectionId) -> Result<Arc<RwLock<OracleConnection>>> {
        self.connections
            .get(conn_id)
            .map(|entry| entry.clone())
            .ok_or_else(|| SakiError::ConnectionNotFound(conn_id.0.to_string()))
    }

    async fn execute_statement(conn: Arc<RwLock<OracleConnection>>, statement: String) -> Result<()> {
        let statement = statement.trim().to_string();
        if statement.is_empty()
            || statement.starts_with("--")
            || statement.starts_with("/*")
            || statement.starts_with("REM")
            || statement.starts_with("PROMPT")
            || statement.starts_with("SET ")
            || statement.starts_with("WHENEVER ")
        {
            return Ok(());
        }

        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_read();
            if statement.trim_start().to_uppercase().starts_with("SELECT") {
                conn.query(&statement, &[])
                    .map_err(|e| SakiError::QueryFailed(format!("Oracle statement failed: {}", e)))?;
            } else {
                conn.execute(&statement, &[])
                    .map_err(|e| SakiError::QueryFailed(format!("Oracle statement failed: {}", e)))?;
                // Ensure commit for DML in restore
                conn.commit()
                    .map_err(|e| SakiError::QueryFailed(format!("Oracle commit failed: {}", e)))?;
            }
            Ok::<(), SakiError>(())
        })
        .await
        .map_err(|e| SakiError::QueryFailed(format!("Statement task failed: {}", e)))?
    }

    pub async fn restore(
        &self,
        conn_id: &ConnectionId,
        file_path: &str,
        options: &RestoreOptions,
        cancelled: &AtomicBool,
        on_progress: Box<dyn for<'a> Fn(&'a RestoreProgress) + Send + Sync>,
    ) -> Result<RestoreProgress> {
        let conn = self.get_connection(conn_id)?;

        // Read file content
        let sql_content = tokio::fs::read_to_string(file_path)
            .await
            .map_err(|e| SakiError::QueryFailed(format!("Failed to read SQL file: {}", e)))?;

        let statements = split_sql_statements(&sql_content);
        let total_statements = statements.len();
        let total_bytes = sql_content.len() as u64;

        info!("Starting Oracle restore with {} statements from {}", total_statements, file_path);

        // Set schema if specified
        if let Some(schema) = &options.schema {
            // Quote the identifier to prevent SQL injection
            let use_schema = format!("ALTER SESSION SET CURRENT_SCHEMA = \"{}\"", schema.replace('"', "\"\""));
            Self::execute_statement(conn.clone(), use_schema).await?;
        }

        let mut bytes_read = 0u64;
        let mut statements_executed = 0u64;
        let mut errors_skipped = 0u64;
        let mut error_messages = Vec::new();
        let start_time = std::time::Instant::now();

        for (index, statement) in statements.iter().enumerate() {
            if cancelled.load(Ordering::Relaxed) {
                info!("Restore cancelled by user");
                break;
            }

            bytes_read += statement.len() as u64;

            match Self::execute_statement(conn.clone(), statement.clone()).await {
                Ok(_) => {
                    statements_executed += 1;
                    debug!("Executed statement {}: {}", index + 1, statement.lines().next().unwrap_or(""));
                }
                Err(e) => {
                    error!("Error at statement {}: {}", index + 1, e);
                    errors_skipped += 1;
                    error_messages.push(format!("Statement {}: {}", index + 1, e));
                    if !options.continue_on_error {
                        let progress = RestoreProgress {
                            bytes_read,
                            total_bytes,
                            statements_executed,
                            errors_skipped,
                            phase: "Failed".to_string(),
                            elapsed_ms: start_time.elapsed().as_millis() as u64,
                            error: Some(e.to_string()),
                            error_messages,
                        };
                        return Err(SakiError::QueryFailed(progress.error.clone().unwrap_or_default()));
                    }
                }
            }

            if index % 10 == 0 || index == total_statements - 1 {
                let progress = RestoreProgress {
                    bytes_read,
                    total_bytes,
                    statements_executed,
                    errors_skipped,
                    phase: format!("Executing ({}/{})", index + 1, total_statements),
                    elapsed_ms: start_time.elapsed().as_millis() as u64,
                    error: None,
                    error_messages: error_messages.clone(),
                };
                on_progress(&progress);
            }
        }

        let final_progress = RestoreProgress {
            bytes_read,
            total_bytes,
            statements_executed,
            errors_skipped,
            phase: "Completed".to_string(),
            elapsed_ms: start_time.elapsed().as_millis() as u64,
            error: None,
            error_messages,
        };
        on_progress(&final_progress);

        info!("Oracle restore complete: {} statements, {} errors", statements_executed, errors_skipped);

        Ok(final_progress)
    }
}
