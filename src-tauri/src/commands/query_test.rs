use sakidb_core::types::ConnectionId;

use super::mock_helpers::*;

// ── Connection ID parsing tests (exercises the common path in query commands) ──

#[tokio::test]
async fn parse_valid_connection_id() {
    let uuid = uuid::Uuid::new_v4();
    let id_str = uuid.to_string();
    let parsed = uuid::Uuid::parse_str(&id_str).unwrap();
    let conn_id = ConnectionId(parsed);
    assert_eq!(conn_id.0, uuid);
}

#[tokio::test]
async fn parse_invalid_connection_id_returns_error() {
    let result = uuid::Uuid::parse_str("invalid-uuid-string");
    assert!(result.is_err());
}

#[tokio::test]
async fn parse_empty_connection_id_returns_error() {
    let result = uuid::Uuid::parse_str("");
    assert!(result.is_err());
}

// ── Registry-based query routing tests ──
// Without a real driver registered, the registry should return errors for unknown connection IDs.

#[tokio::test]
async fn sql_for_unknown_connection_fails() {
    let (state, _tmp) = create_test_state();
    let conn_id = ConnectionId(uuid::Uuid::new_v4());

    let result = state.registry.sql_for(&conn_id);
    assert!(result.is_err());
}

#[tokio::test]
async fn disconnect_unknown_connection_fails() {
    let (state, _tmp) = create_test_state();
    let conn_id = ConnectionId(uuid::Uuid::new_v4());

    let result = state.registry.disconnect(&conn_id).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn driver_for_unknown_connection_fails() {
    let (state, _tmp) = create_test_state();
    let conn_id = ConnectionId(uuid::Uuid::new_v4());

    let result = state.registry.driver_for(&conn_id);
    assert!(result.is_err());
}

#[tokio::test]
async fn capabilities_for_unknown_connection_fails() {
    let (state, _tmp) = create_test_state();
    let conn_id = ConnectionId(uuid::Uuid::new_v4());

    let result = state.registry.capabilities_for(&conn_id);
    assert!(result.is_err());
}
