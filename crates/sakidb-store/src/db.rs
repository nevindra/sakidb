use rusqlite::{params, Connection};
use tracing::{debug, info};
use uuid::Uuid;

use sakidb_core::SakiError;
use crate::models::{ConnectionInput, QueryHistoryEntry, SavedConnection, SavedQuery};

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(db_path: &str) -> Result<Self, SakiError> {
        debug!(path = db_path, "opening store");
        let conn = Connection::open(db_path)
            .map_err(|e| SakiError::StorageError(e.to_string()))?;
        let store = Self { conn };
        store.init_tables()?;
        info!(path = db_path, "store opened");
        Ok(store)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self, SakiError> {
        let conn = Connection::open_in_memory()
            .map_err(|e| SakiError::StorageError(e.to_string()))?;
        let store = Self { conn };
        store.init_tables()?;
        Ok(store)
    }

    fn init_tables(&self) -> Result<(), SakiError> {
        self.conn
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS connections (
                    id          TEXT PRIMARY KEY,
                    name        TEXT NOT NULL,
                    engine      TEXT NOT NULL DEFAULT 'postgres',
                    host        TEXT NOT NULL,
                    port        INTEGER NOT NULL DEFAULT 5432,
                    database    TEXT NOT NULL,
                    username    TEXT NOT NULL,
                    password    TEXT NOT NULL DEFAULT '',
                    ssl_mode    TEXT NOT NULL DEFAULT 'prefer',
                    options     TEXT NOT NULL DEFAULT '{}',
                    created_at  TEXT NOT NULL,
                    updated_at  TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS app_config (
                    key   TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS saved_queries (
                    id              TEXT PRIMARY KEY,
                    name            TEXT NOT NULL,
                    sql             TEXT NOT NULL,
                    connection_id   TEXT,
                    database_name   TEXT,
                    created_at      TEXT NOT NULL,
                    updated_at      TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS query_history (
                    id                TEXT PRIMARY KEY,
                    sql               TEXT NOT NULL,
                    connection_id     TEXT,
                    database_name     TEXT,
                    executed_at       TEXT NOT NULL,
                    execution_time_ms INTEGER,
                    row_count         INTEGER
                );
                CREATE TABLE IF NOT EXISTS keybindings (
                    command_id  TEXT PRIMARY KEY,
                    keybinding  TEXT
                );",
            )
            .map_err(|e| SakiError::StorageError(e.to_string()))?;

        // Migration: add last_connected_at column if missing
        let has_column: bool = self.conn
            .prepare("SELECT last_connected_at FROM connections LIMIT 0")
            .is_ok();
        if !has_column {
            self.conn
                .execute_batch("ALTER TABLE connections ADD COLUMN last_connected_at TEXT;")
                .map_err(|e| SakiError::StorageError(e.to_string()))?;
        }

        // Migration: add engine column if missing (defaults existing rows to 'postgres')
        let has_engine: bool = self.conn
            .prepare("SELECT engine FROM connections LIMIT 0")
            .is_ok();
        if !has_engine {
            self.conn
                .execute_batch("ALTER TABLE connections ADD COLUMN engine TEXT NOT NULL DEFAULT 'postgres';")
                .map_err(|e| SakiError::StorageError(e.to_string()))?;
        }

        // [Fix: M7] Migration: add options column if missing to support engine-specific settings
        let has_options: bool = self.conn
            .prepare("SELECT options FROM connections LIMIT 0")
            .is_ok();
        if !has_options {
            self.conn
                .execute_batch("ALTER TABLE connections ADD COLUMN options TEXT NOT NULL DEFAULT '{}';")
                .map_err(|e| SakiError::StorageError(e.to_string()))?;
        }

        // Migration: if password column was BLOB (old encrypted format), clear them out.
        // SQLite doesn't support ALTER COLUMN, so we detect old rows by checking type affinity.
        // Old encrypted passwords were stored as BLOB; new ones are TEXT.
        // Clear any BLOB passwords since they can't be decrypted without the old master password.
        self.conn
            .execute("UPDATE connections SET password = '' WHERE typeof(password) = 'blob'", [])
            .map_err(|e| SakiError::StorageError(e.to_string()))?;

        Ok(())
    }

    pub fn save_connection(&self, input: &ConnectionInput) -> Result<SavedConnection, SakiError> {
        debug!(name = %input.name, host = %input.host, "saving connection");
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let options_json = serde_json::to_string(&input.options).unwrap_or_else(|_| "{}".to_string());

        self.conn
            .execute(
                "INSERT INTO connections (id, name, engine, host, port, database, username, password, ssl_mode, options, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![id, input.name, input.engine, input.host, input.port, input.database, input.username, input.password, input.ssl_mode, options_json, now, now],
            )
            .map_err(|e| SakiError::StorageError(e.to_string()))?;

        Ok(SavedConnection {
            id,
            name: input.name.clone(),
            engine: input.engine.clone(),
            host: input.host.clone(),
            port: input.port,
            database: input.database.clone(),
            username: input.username.clone(),
            password: input.password.clone(),
            ssl_mode: input.ssl_mode.clone(),
            options: input.options.clone(),
            created_at: now.clone(),
            updated_at: now,
            last_connected_at: None,
        })
    }

    pub fn list_connections(&self) -> Result<Vec<SavedConnection>, SakiError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, engine, host, port, database, username, password, ssl_mode, created_at, updated_at, last_connected_at, options FROM connections ORDER BY name")
            .map_err(|e| SakiError::StorageError(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                let options_json: String = row.get(12).unwrap_or_else(|_| "{}".to_string());
                let options = serde_json::from_str(&options_json).unwrap_or_default();
                Ok(SavedConnection {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    engine: row.get(2)?,
                    host: row.get(3)?,
                    port: row.get::<_, i32>(4)? as u16,
                    database: row.get(5)?,
                    username: row.get(6)?,
                    password: row.get(7)?,
                    ssl_mode: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                    last_connected_at: row.get(11)?,
                    options,
                })
            })
            .map_err(|e| SakiError::StorageError(e.to_string()))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| SakiError::StorageError(e.to_string()))
    }

    pub fn get_connection(&self, id: &str) -> Result<SavedConnection, SakiError> {
        self.conn
            .query_row(
                "SELECT id, name, engine, host, port, database, username, password, ssl_mode, created_at, updated_at, last_connected_at, options FROM connections WHERE id = ?1",
                params![id],
                |row| {
                    let options_json: String = row.get(12).unwrap_or_else(|_| "{}".to_string());
                    let options = serde_json::from_str(&options_json).unwrap_or_default();
                    Ok(SavedConnection {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        engine: row.get(2)?,
                        host: row.get(3)?,
                        port: row.get::<_, i32>(4)? as u16,
                        database: row.get(5)?,
                        username: row.get(6)?,
                        password: row.get(7)?,
                        ssl_mode: row.get(8)?,
                        created_at: row.get(9)?,
                        updated_at: row.get(10)?,
                        last_connected_at: row.get(11)?,
                        options,
                    })
                },
            )
            .map_err(|e| SakiError::StorageError(e.to_string()))
    }

    pub fn update_connection(&self, id: &str, input: &ConnectionInput) -> Result<(), SakiError> {
        let now = chrono::Utc::now().to_rfc3339();
        let options_json = serde_json::to_string(&input.options).unwrap_or_else(|_| "{}".to_string());

        let affected = if input.password.is_empty() {
            // Don't overwrite stored password when none provided
            self.conn.execute(
                "UPDATE connections SET name=?1, engine=?2, host=?3, port=?4, database=?5, username=?6, ssl_mode=?7, options=?8, updated_at=?9 WHERE id=?10",
                params![input.name, input.engine, input.host, input.port, input.database, input.username, input.ssl_mode, options_json, now, id],
            )
        } else {
            self.conn.execute(
                "UPDATE connections SET name=?1, engine=?2, host=?3, port=?4, database=?5, username=?6, password=?7, ssl_mode=?8, options=?9, updated_at=?10 WHERE id=?11",
                params![input.name, input.engine, input.host, input.port, input.database, input.username, input.password, input.ssl_mode, options_json, now, id],
            )
        }
        .map_err(|e| SakiError::StorageError(e.to_string()))?;

        if affected == 0 {
            return Err(SakiError::ConnectionNotFound(id.to_string()));
        }
        Ok(())
    }

    pub fn delete_connection(&self, id: &str) -> Result<(), SakiError> {
        debug!(id, "deleting connection");
        let affected = self
            .conn
            .execute("DELETE FROM connections WHERE id = ?1", params![id])
            .map_err(|e| SakiError::StorageError(e.to_string()))?;

        if affected == 0 {
            return Err(SakiError::ConnectionNotFound(id.to_string()));
        }
        Ok(())
    }

    pub fn update_last_connected(&self, id: &str) -> Result<(), SakiError> {
        let now = chrono::Utc::now().to_rfc3339();
        let affected = self
            .conn
            .execute(
                "UPDATE connections SET last_connected_at = ?1 WHERE id = ?2",
                params![now, id],
            )
            .map_err(|e| SakiError::StorageError(e.to_string()))?;

        if affected == 0 {
            return Err(SakiError::ConnectionNotFound(id.to_string()));
        }
        Ok(())
    }

    // ── Saved queries ──

    pub fn save_query(
        &self,
        name: &str,
        sql: &str,
        connection_id: Option<&str>,
        database_name: Option<&str>,
    ) -> Result<SavedQuery, SakiError> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        self.conn
            .execute(
                "INSERT INTO saved_queries (id, name, sql, connection_id, database_name, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![id, name, sql, connection_id, database_name, now, now],
            )
            .map_err(|e| SakiError::StorageError(e.to_string()))?;

        Ok(SavedQuery {
            id,
            name: name.to_string(),
            sql: sql.to_string(),
            connection_id: connection_id.map(String::from),
            database_name: database_name.map(String::from),
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub fn list_saved_queries(&self) -> Result<Vec<SavedQuery>, SakiError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, sql, connection_id, database_name, created_at, updated_at FROM saved_queries ORDER BY updated_at DESC")
            .map_err(|e| SakiError::StorageError(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(SavedQuery {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    sql: row.get(2)?,
                    connection_id: row.get(3)?,
                    database_name: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })
            .map_err(|e| SakiError::StorageError(e.to_string()))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| SakiError::StorageError(e.to_string()))
    }

    pub fn update_saved_query(
        &self,
        id: &str,
        name: Option<&str>,
        sql: Option<&str>,
    ) -> Result<SavedQuery, SakiError> {
        let now = chrono::Utc::now().to_rfc3339();

        // Fetch current to merge partial updates
        let current: SavedQuery = self
            .conn
            .query_row(
                "SELECT id, name, sql, connection_id, database_name, created_at, updated_at FROM saved_queries WHERE id = ?1",
                params![id],
                |row| {
                    Ok(SavedQuery {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        sql: row.get(2)?,
                        connection_id: row.get(3)?,
                        database_name: row.get(4)?,
                        created_at: row.get(5)?,
                        updated_at: row.get(6)?,
                    })
                },
            )
            .map_err(|e| SakiError::StorageError(e.to_string()))?;

        let new_name = name.unwrap_or(&current.name);
        let new_sql = sql.unwrap_or(&current.sql);

        self.conn
            .execute(
                "UPDATE saved_queries SET name = ?1, sql = ?2, updated_at = ?3 WHERE id = ?4",
                params![new_name, new_sql, now, id],
            )
            .map_err(|e| SakiError::StorageError(e.to_string()))?;

        Ok(SavedQuery {
            id: id.to_string(),
            name: new_name.to_string(),
            sql: new_sql.to_string(),
            connection_id: current.connection_id,
            database_name: current.database_name,
            created_at: current.created_at,
            updated_at: now,
        })
    }

    pub fn delete_saved_query(&self, id: &str) -> Result<(), SakiError> {
        let affected = self
            .conn
            .execute("DELETE FROM saved_queries WHERE id = ?1", params![id])
            .map_err(|e| SakiError::StorageError(e.to_string()))?;

        if affected == 0 {
            return Err(SakiError::StorageError(format!("Saved query not found: {}", id)));
        }
        Ok(())
    }

    // ── Query history ──

    pub fn add_query_history(
        &self,
        sql: &str,
        connection_id: Option<&str>,
        database_name: Option<&str>,
        execution_time_ms: Option<i64>,
        row_count: Option<i64>,
    ) -> Result<QueryHistoryEntry, SakiError> {
        let now = chrono::Utc::now().to_rfc3339();

        // Dedup: if identical sql + connection_id + database_name exists, update it
        let existing_id: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM query_history WHERE sql = ?1 AND connection_id IS ?2 AND database_name IS ?3",
                params![sql, connection_id, database_name],
                |row| row.get(0),
            )
            .ok();

        if let Some(eid) = existing_id {
            self.conn
                .execute(
                    "UPDATE query_history SET executed_at = ?1, execution_time_ms = ?2, row_count = ?3 WHERE id = ?4",
                    params![now, execution_time_ms, row_count, eid],
                )
                .map_err(|e| SakiError::StorageError(e.to_string()))?;

            return Ok(QueryHistoryEntry {
                id: eid,
                sql: sql.to_string(),
                connection_id: connection_id.map(String::from),
                database_name: database_name.map(String::from),
                executed_at: now,
                execution_time_ms,
                row_count,
            });
        }

        let id = Uuid::new_v4().to_string();

        self.conn
            .execute(
                "INSERT INTO query_history (id, sql, connection_id, database_name, executed_at, execution_time_ms, row_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![id, sql, connection_id, database_name, now, execution_time_ms, row_count],
            )
            .map_err(|e| SakiError::StorageError(e.to_string()))?;

        // Purge oldest entries beyond 100
        self.conn
            .execute(
                "DELETE FROM query_history WHERE id NOT IN (SELECT id FROM query_history ORDER BY executed_at DESC LIMIT 100)",
                [],
            )
            .map_err(|e| SakiError::StorageError(e.to_string()))?;

        Ok(QueryHistoryEntry {
            id,
            sql: sql.to_string(),
            connection_id: connection_id.map(String::from),
            database_name: database_name.map(String::from),
            executed_at: now,
            execution_time_ms,
            row_count,
        })
    }

    pub fn list_query_history(&self, limit: Option<u32>) -> Result<Vec<QueryHistoryEntry>, SakiError> {
        let limit = limit.unwrap_or(100);
        let mut stmt = self
            .conn
            .prepare("SELECT id, sql, connection_id, database_name, executed_at, execution_time_ms, row_count FROM query_history ORDER BY executed_at DESC LIMIT ?1")
            .map_err(|e| SakiError::StorageError(e.to_string()))?;

        let rows = stmt
            .query_map(params![limit], |row| {
                Ok(QueryHistoryEntry {
                    id: row.get(0)?,
                    sql: row.get(1)?,
                    connection_id: row.get(2)?,
                    database_name: row.get(3)?,
                    executed_at: row.get(4)?,
                    execution_time_ms: row.get(5)?,
                    row_count: row.get(6)?,
                })
            })
            .map_err(|e| SakiError::StorageError(e.to_string()))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| SakiError::StorageError(e.to_string()))
    }

    pub fn clear_query_history(&self) -> Result<(), SakiError> {
        self.conn
            .execute("DELETE FROM query_history", [])
            .map_err(|e| SakiError::StorageError(e.to_string()))?;
        Ok(())
    }

    // ── Keybindings ──

    pub fn get_keybinding_overrides(&self) -> Result<Vec<(String, Option<String>)>, SakiError> {
        let mut stmt = self
            .conn
            .prepare("SELECT command_id, keybinding FROM keybindings")
            .map_err(|e| SakiError::StorageError(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| SakiError::StorageError(e.to_string()))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| SakiError::StorageError(e.to_string()))
    }

    pub fn set_keybinding(&self, command_id: &str, keybinding: Option<&str>) -> Result<(), SakiError> {
        self.conn
            .execute(
                "INSERT INTO keybindings (command_id, keybinding) VALUES (?1, ?2)
                 ON CONFLICT(command_id) DO UPDATE SET keybinding = excluded.keybinding",
                params![command_id, keybinding],
            )
            .map_err(|e| SakiError::StorageError(e.to_string()))?;
        Ok(())
    }

    pub fn reset_keybinding(&self, command_id: &str) -> Result<(), SakiError> {
        self.conn
            .execute("DELETE FROM keybindings WHERE command_id = ?1", params![command_id])
            .map_err(|e| SakiError::StorageError(e.to_string()))?;
        Ok(())
    }

    pub fn reset_all_keybindings(&self) -> Result<(), SakiError> {
        self.conn
            .execute("DELETE FROM keybindings", [])
            .map_err(|e| SakiError::StorageError(e.to_string()))?;
        Ok(())
    }

    pub fn set_preference(&self, key: &str, value: &str) -> Result<(), SakiError> {
        self.conn
            .execute(
                "INSERT INTO app_config (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )
            .map_err(|e| SakiError::StorageError(e.to_string()))?;
        Ok(())
    }

    pub fn get_preference(&self, key: &str) -> Result<Option<String>, SakiError> {
        use rusqlite::OptionalExtension;
        self.conn
            .query_row(
                "SELECT value FROM app_config WHERE key = ?1",
                params![key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| SakiError::StorageError(e.to_string()))
    }

    pub fn save_from_history(&self, history_id: &str, name: &str) -> Result<SavedQuery, SakiError> {
        let entry: QueryHistoryEntry = self
            .conn
            .query_row(
                "SELECT id, sql, connection_id, database_name, executed_at, execution_time_ms, row_count FROM query_history WHERE id = ?1",
                params![history_id],
                |row| {
                    Ok(QueryHistoryEntry {
                        id: row.get(0)?,
                        sql: row.get(1)?,
                        connection_id: row.get(2)?,
                        database_name: row.get(3)?,
                        executed_at: row.get(4)?,
                        execution_time_ms: row.get(5)?,
                        row_count: row.get(6)?,
                    })
                },
            )
            .map_err(|e| SakiError::StorageError(e.to_string()))?;

        self.save_query(
            name,
            &entry.sql,
            entry.connection_id.as_deref(),
            entry.database_name.as_deref(),
        )
    }
}

