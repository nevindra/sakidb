//! Test helpers for Tauri command tests.
//!
//! Provides utility functions to create test `AppState` instances
//! backed by temporary SQLite stores, enabling integration testing
//! of all store-based Tauri commands without a running database server.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use dashmap::DashMap;
use sakidb_store::Store;
use tokio::sync::Mutex;

use crate::registry::DriverRegistry;
use crate::state::AppState;

/// Create a test `AppState` backed by a temporary SQLite database.
///
/// The store uses a temp file so it persists for the duration of the test
/// (unlike in-memory stores which are only available inside the sakidb-store crate).
///
/// The registry is empty (no real drivers registered) since we test store operations directly.
///
/// Returns `(AppState, TempDir)` — caller must hold `TempDir` alive so the file isn't deleted.
pub(crate) fn create_test_state() -> (AppState, tempfile::TempDir) {
    let tmp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let db_path = tmp_dir.path().join("test_config.db");
    let store =
        Store::open(db_path.to_str().expect("invalid path")).expect("failed to open test store");

    let state = AppState {
        registry: Arc::new(DriverRegistry::new()),
        store: Arc::new(Mutex::new(store)),
        restore_cancelled: Arc::new(AtomicBool::new(false)),
        export_cancel_flags: Arc::new(DashMap::new()),
    };

    (state, tmp_dir)
}

/// Create a test `ConnectionInput` with sensible defaults.
pub(crate) fn test_connection_input() -> sakidb_store::models::ConnectionInput {
    sakidb_store::models::ConnectionInput {
        name: "Test Connection".to_string(),
        engine: "postgres".to_string(),
        host: "localhost".to_string(),
        port: 5432,
        database: "testdb".to_string(),
        username: "testuser".to_string(),
        password: "testpass".to_string(),
        ssl_mode: "prefer".to_string(),
        options: std::collections::HashMap::new(),
    }
}

/// Save a connection to the store and return its saved ID.
#[allow(dead_code)]
pub(crate) async fn save_test_connection(state: &AppState) -> String {
    let store = state.store.lock().await;
    let saved = store
        .save_connection(&test_connection_input())
        .expect("failed to save test connection");
    saved.id
}
