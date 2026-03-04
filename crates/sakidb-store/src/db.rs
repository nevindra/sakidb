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
                    host        TEXT NOT NULL,
                    port        INTEGER NOT NULL DEFAULT 5432,
                    database    TEXT NOT NULL,
                    username    TEXT NOT NULL,
                    password    TEXT NOT NULL DEFAULT '',
                    ssl_mode    TEXT NOT NULL DEFAULT 'prefer',
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

        self.conn
            .execute(
                "INSERT INTO connections (id, name, host, port, database, username, password, ssl_mode, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![id, input.name, input.host, input.port, input.database, input.username, input.password, input.ssl_mode, now, now],
            )
            .map_err(|e| SakiError::StorageError(e.to_string()))?;

        Ok(SavedConnection {
            id,
            name: input.name.clone(),
            host: input.host.clone(),
            port: input.port,
            database: input.database.clone(),
            username: input.username.clone(),
            password: input.password.clone(),
            ssl_mode: input.ssl_mode.clone(),
            created_at: now.clone(),
            updated_at: now,
            last_connected_at: None,
        })
    }

    pub fn list_connections(&self) -> Result<Vec<SavedConnection>, SakiError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, host, port, database, username, password, ssl_mode, created_at, updated_at, last_connected_at FROM connections ORDER BY name")
            .map_err(|e| SakiError::StorageError(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(SavedConnection {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    host: row.get(2)?,
                    port: row.get::<_, i32>(3)? as u16,
                    database: row.get(4)?,
                    username: row.get(5)?,
                    password: row.get(6)?,
                    ssl_mode: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                    last_connected_at: row.get(10)?,
                })
            })
            .map_err(|e| SakiError::StorageError(e.to_string()))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| SakiError::StorageError(e.to_string()))
    }

    pub fn get_connection(&self, id: &str) -> Result<SavedConnection, SakiError> {
        self.conn
            .query_row(
                "SELECT id, name, host, port, database, username, password, ssl_mode, created_at, updated_at, last_connected_at FROM connections WHERE id = ?1",
                params![id],
                |row| {
                    Ok(SavedConnection {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        host: row.get(2)?,
                        port: row.get::<_, i32>(3)? as u16,
                        database: row.get(4)?,
                        username: row.get(5)?,
                        password: row.get(6)?,
                        ssl_mode: row.get(7)?,
                        created_at: row.get(8)?,
                        updated_at: row.get(9)?,
                        last_connected_at: row.get(10)?,
                    })
                },
            )
            .map_err(|e| SakiError::StorageError(e.to_string()))
    }

    pub fn update_connection(&self, id: &str, input: &ConnectionInput) -> Result<(), SakiError> {
        let now = chrono::Utc::now().to_rfc3339();

        let affected = if input.password.is_empty() {
            // Don't overwrite stored password when none provided
            self.conn.execute(
                "UPDATE connections SET name=?1, host=?2, port=?3, database=?4, username=?5, ssl_mode=?6, updated_at=?7 WHERE id=?8",
                params![input.name, input.host, input.port, input.database, input.username, input.ssl_mode, now, id],
            )
        } else {
            self.conn.execute(
                "UPDATE connections SET name=?1, host=?2, port=?3, database=?4, username=?5, password=?6, ssl_mode=?7, updated_at=?8 WHERE id=?9",
                params![input.name, input.host, input.port, input.database, input.username, input.password, input.ssl_mode, now, id],
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ConnectionInput;

    fn test_input() -> ConnectionInput {
        ConnectionInput {
            name: "Test DB".into(),
            host: "localhost".into(),
            port: 5432,
            database: "testdb".into(),
            username: "user".into(),
            password: "secret123".into(),
            ssl_mode: "prefer".into(),
        }
    }

    #[test]
    fn crud_roundtrip() {
        let store = Store::open_in_memory().unwrap();

        let saved = store.save_connection(&test_input()).unwrap();
        assert_eq!(saved.name, "Test DB");

        let list = store.list_connections().unwrap();
        assert_eq!(list.len(), 1);

        let fetched = store.get_connection(&saved.id).unwrap();
        assert_eq!(fetched.host, "localhost");
        assert_eq!(fetched.password, "secret123");

        let mut updated_input = test_input();
        updated_input.name = "Updated DB".into();
        store.update_connection(&saved.id, &updated_input).unwrap();
        let fetched = store.get_connection(&saved.id).unwrap();
        assert_eq!(fetched.name, "Updated DB");

        // Empty password should not overwrite stored password
        let mut partial_input = test_input();
        partial_input.name = "Renamed Again".into();
        partial_input.password = String::new();
        store.update_connection(&saved.id, &partial_input).unwrap();
        let fetched = store.get_connection(&saved.id).unwrap();
        assert_eq!(fetched.name, "Renamed Again");
        assert_eq!(fetched.password, "secret123");

        store.delete_connection(&saved.id).unwrap();
        let list = store.list_connections().unwrap();
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn delete_nonexistent_fails() {
        let store = Store::open_in_memory().unwrap();
        let result = store.delete_connection("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn saved_queries_crud() {
        let store = Store::open_in_memory().unwrap();

        let q = store.save_query("My Query", "SELECT 1", Some("conn-1"), Some("mydb")).unwrap();
        assert_eq!(q.name, "My Query");
        assert_eq!(q.sql, "SELECT 1");

        let list = store.list_saved_queries().unwrap();
        assert_eq!(list.len(), 1);

        let updated = store.update_saved_query(&q.id, Some("Renamed"), None).unwrap();
        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.sql, "SELECT 1");

        store.delete_saved_query(&q.id).unwrap();
        assert_eq!(store.list_saved_queries().unwrap().len(), 0);
    }

    #[test]
    fn query_history_dedup() {
        let store = Store::open_in_memory().unwrap();

        let e1 = store.add_query_history("SELECT 1", Some("c1"), Some("db1"), Some(10), Some(1)).unwrap();
        let e2 = store.add_query_history("SELECT 1", Some("c1"), Some("db1"), Some(20), Some(1)).unwrap();

        // Same id due to dedup
        assert_eq!(e1.id, e2.id);
        assert_eq!(e2.execution_time_ms, Some(20));

        let list = store.list_query_history(None).unwrap();
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn save_from_history_works() {
        let store = Store::open_in_memory().unwrap();

        let entry = store.add_query_history("SELECT * FROM users", Some("c1"), Some("db1"), Some(50), Some(10)).unwrap();
        let saved = store.save_from_history(&entry.id, "Users query").unwrap();

        assert_eq!(saved.name, "Users query");
        assert_eq!(saved.sql, "SELECT * FROM users");
        assert_eq!(saved.connection_id, Some("c1".to_string()));
    }
}
