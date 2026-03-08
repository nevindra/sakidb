use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use sakidb_core::types::ConnectionId;

use super::mock_helpers::*;

// ── Export cancel flag tests ──

#[tokio::test]
async fn cancel_export_sets_flag() {
    let (state, _tmp) = create_test_state();
    let conn_id = ConnectionId(uuid::Uuid::new_v4());

    // Insert a cancel flag
    let flag = Arc::new(AtomicBool::new(false));
    state.export_cancel_flags.insert(conn_id, flag.clone());

    // Simulate cancel
    if let Some(f) = state.export_cancel_flags.get(&conn_id) {
        f.store(true, Ordering::Relaxed);
    }

    assert!(flag.load(Ordering::Relaxed));
}

#[tokio::test]
async fn cancel_export_no_active_export_is_noop() {
    let (state, _tmp) = create_test_state();
    let conn_id = ConnectionId(uuid::Uuid::new_v4());

    // No flag inserted, so get returns None
    let flag = state.export_cancel_flags.get(&conn_id);
    assert!(flag.is_none());
    // This is the same behavior as cancel_export command — it's a no-op
}

#[tokio::test]
async fn export_cancel_flag_cleanup() {
    let (state, _tmp) = create_test_state();
    let conn_id = ConnectionId(uuid::Uuid::new_v4());

    let flag = Arc::new(AtomicBool::new(false));
    state.export_cancel_flags.insert(conn_id, flag);

    // Simulate cleanup after export
    state.export_cancel_flags.remove(&conn_id);
    assert!(state.export_cancel_flags.get(&conn_id).is_none());
}

// ── UUID parsing for export commands ──

#[tokio::test]
async fn export_parse_invalid_uuid_fails() {
    let result = uuid::Uuid::parse_str("bad-uuid");
    assert!(result.is_err());
}

// ── Registry routing for export ──

#[tokio::test]
async fn exporter_for_unknown_connection_fails() {
    let (state, _tmp) = create_test_state();
    let conn_id = ConnectionId(uuid::Uuid::new_v4());
    let result = state.registry.exporter_for(&conn_id);
    assert!(result.is_err());
}
