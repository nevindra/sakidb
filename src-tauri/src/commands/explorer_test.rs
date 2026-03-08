use sakidb_core::types::{ConnectionId, EngineType};

use super::mock_helpers::*;

// ── Explorer commands route through the registry to driver introspection methods.
// Without a real driver registered, the registry returns "connection not found"
// or "not supported" errors. We verify the routing and error paths. ──

#[tokio::test]
async fn introspector_for_unknown_connection_fails() {
    let (state, _tmp) = create_test_state();
    let conn_id = ConnectionId(uuid::Uuid::new_v4());
    let result = state.registry.introspector_for(&conn_id);
    assert!(result.is_err());
}

#[tokio::test]
async fn sql_for_unknown_connection_fails() {
    let (state, _tmp) = create_test_state();
    let conn_id = ConnectionId(uuid::Uuid::new_v4());
    let result = state.registry.sql_for(&conn_id);
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
async fn driver_by_unregistered_engine_fails() {
    let (state, _tmp) = create_test_state();
    // Empty registry has no drivers registered
    let result = state.registry.driver_by_engine(&EngineType::Postgres);
    assert!(result.is_err());
}

#[tokio::test]
async fn exporter_for_unknown_connection_fails() {
    let (state, _tmp) = create_test_state();
    let conn_id = ConnectionId(uuid::Uuid::new_v4());
    let result = state.registry.exporter_for(&conn_id);
    assert!(result.is_err());
}

#[tokio::test]
async fn restorer_for_unknown_connection_fails() {
    let (state, _tmp) = create_test_state();
    let conn_id = ConnectionId(uuid::Uuid::new_v4());
    let result = state.registry.restorer_for(&conn_id);
    assert!(result.is_err());
}

// ── UUID parsing tests for explorer parse_conn_id ──

#[tokio::test]
async fn explorer_parse_valid_uuid() {
    let uuid_str = uuid::Uuid::new_v4().to_string();
    let parsed = uuid::Uuid::parse_str(&uuid_str);
    assert!(parsed.is_ok());
}

#[tokio::test]
async fn explorer_parse_invalid_uuid_fails() {
    let result = uuid::Uuid::parse_str("not-valid");
    assert!(result.is_err());
}
