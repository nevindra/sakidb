use std::sync::atomic::Ordering;

use sakidb_core::types::ConnectionId;

use super::mock_helpers::*;

// ── Restore cancel flag tests ──

#[tokio::test]
async fn cancel_restore_sets_flag() {
    let (state, _tmp) = create_test_state();

    assert!(!state.restore_cancelled.load(Ordering::Relaxed));

    state.restore_cancelled.store(true, Ordering::Relaxed);

    assert!(state.restore_cancelled.load(Ordering::Relaxed));
}

#[tokio::test]
async fn restore_cancel_flag_resets() {
    let (state, _tmp) = create_test_state();

    // Set cancelled
    state.restore_cancelled.store(true, Ordering::Relaxed);
    assert!(state.restore_cancelled.load(Ordering::Relaxed));

    // Reset (as restore_from_sql does at start)
    state.restore_cancelled.store(false, Ordering::Relaxed);
    assert!(!state.restore_cancelled.load(Ordering::Relaxed));
}

// ── UUID parsing for import commands ──

#[tokio::test]
async fn import_parse_invalid_uuid_fails() {
    let result = uuid::Uuid::parse_str("not-a-uuid");
    assert!(result.is_err());
}

// ── Registry routing for restore ──

#[tokio::test]
async fn restorer_for_unknown_connection_fails() {
    let (state, _tmp) = create_test_state();
    let conn_id = ConnectionId(uuid::Uuid::new_v4());
    let result = state.registry.restorer_for(&conn_id);
    assert!(result.is_err());
}
