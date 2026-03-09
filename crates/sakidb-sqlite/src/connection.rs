use std::path::Path;
use std::sync::{Arc, Mutex};

use dashmap::DashMap;
use rusqlite::{Connection, InterruptHandle, OpenFlags};
use tracing::{debug, info, warn};

use sakidb_core::types::ConnectionId;
use sakidb_core::{Result, SakiError};

/// Detect whether a file path is read-only and return appropriate OpenFlags.
fn open_flags_for(file_path: &str) -> (OpenFlags, bool) {
    let path = Path::new(file_path);
    let read_only = path.exists()
        && std::fs::metadata(path)
            .map(|m| m.permissions().readonly())
            .unwrap_or(false);

    let flags = if read_only {
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
    };

    (flags, read_only)
}

pub struct ConnectionManager {
    connections: DashMap<ConnectionId, Arc<Mutex<Connection>>>,
    interrupt_handles: DashMap<ConnectionId, InterruptHandle>,
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self {
            connections: DashMap::new(),
            interrupt_handles: DashMap::new(),
        }
    }
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn connect(&self, file_path: &str) -> Result<ConnectionId> {
        let (flags, read_only) = open_flags_for(file_path);

        info!(path = %file_path, read_only, "connecting to SQLite database");

        let conn = Connection::open_with_flags(file_path, flags)
            .map_err(|e| SakiError::ConnectionFailed(e.to_string()))?;

        // Apply performance pragmas
        if read_only {
            conn.execute_batch(
                "PRAGMA query_only = ON;
                 PRAGMA mmap_size = 268435456;
                 PRAGMA cache_size = -65536;
                 PRAGMA temp_store = MEMORY;",
            )
            .map_err(|e| SakiError::ConnectionFailed(format!("pragma setup failed: {e}")))?;
        } else {
            conn.execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA mmap_size = 268435456;
                 PRAGMA cache_size = -65536;
                 PRAGMA busy_timeout = 5000;
                 PRAGMA foreign_keys = ON;
                 PRAGMA temp_store = MEMORY;
                 PRAGMA synchronous = NORMAL;",
            )
            .map_err(|e| SakiError::ConnectionFailed(format!("pragma setup failed: {e}")))?;
        }

        let interrupt_handle = conn.get_interrupt_handle();
        let id = ConnectionId::new();

        self.connections.insert(id, Arc::new(Mutex::new(conn)));
        self.interrupt_handles.insert(id, interrupt_handle);

        info!(conn_id = %id.0, path = %file_path, "connected to SQLite");
        Ok(id)
    }

    pub fn disconnect(&self, conn_id: &ConnectionId) -> Result<()> {
        self.interrupt_handles.remove(conn_id);
        if self.connections.remove(conn_id).is_some() {
            info!(conn_id = %conn_id.0, "disconnected from SQLite");
            Ok(())
        } else {
            warn!(conn_id = %conn_id.0, "disconnect: connection not found");
            Err(SakiError::ConnectionNotFound(conn_id.0.to_string()))
        }
    }

    /// Run a blocking closure on the connection inside `spawn_blocking`,
    /// handling the mutex lock and join-error mapping.
    pub async fn with_conn<F, T>(&self, conn_id: &ConnectionId, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let conn = self.get_conn(conn_id)?;
        tokio::task::spawn_blocking(move || {
            let conn = conn
                .lock()
                .map_err(|e| SakiError::QueryFailed(format!("lock poisoned: {e}")))?;
            f(&conn)
        })
        .await
        .map_err(|e| SakiError::QueryFailed(format!("task failed: {e}")))?
    }

    pub fn get_conn(&self, conn_id: &ConnectionId) -> Result<Arc<Mutex<Connection>>> {
        self.connections
            .get(conn_id)
            .map(|c| c.value().clone())
            .ok_or_else(|| SakiError::ConnectionNotFound(conn_id.0.to_string()))
    }

    pub fn interrupt(&self, conn_id: &ConnectionId) -> Result<()> {
        if let Some(handle) = self.interrupt_handles.get(conn_id) {
            handle.interrupt();
            info!(conn_id = %conn_id.0, "query interrupted");
            Ok(())
        } else {
            debug!(conn_id = %conn_id.0, "interrupt: no connection found");
            Ok(())
        }
    }

    pub fn test_connection(file_path: &str) -> Result<()> {
        let (flags, _) = open_flags_for(file_path);

        let conn = Connection::open_with_flags(file_path, flags)
            .map_err(|e| SakiError::ConnectionFailed(e.to_string()))?;

        conn.execute_batch("SELECT 1")
            .map_err(|e| SakiError::ConnectionFailed(e.to_string()))?;

        info!(path = %file_path, "test connection successful");
        Ok(())
    }
}
