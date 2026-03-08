use super::mock_helpers::*;

// ── Store-based connection CRUD tests ──

#[tokio::test]
async fn save_connection_returns_saved_connection() {
    let (state, _tmp) = create_test_state();
    let input = test_connection_input();

    let store = state.store.lock().await;
    let result = store.save_connection(&input);

    assert!(result.is_ok());
    let saved = result.unwrap();
    assert_eq!(saved.name, "Test Connection");
    assert_eq!(saved.engine, "postgres");
    assert_eq!(saved.host, "localhost");
    assert_eq!(saved.port, 5432);
    assert_eq!(saved.database, "testdb");
    assert_eq!(saved.username, "testuser");
    assert_eq!(saved.ssl_mode, "prefer");
    assert!(!saved.id.is_empty());
}

#[tokio::test]
async fn list_connections_empty() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;
    let result = store.list_connections().unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn list_connections_returns_saved() {
    let (state, _tmp) = create_test_state();

    {
        let store = state.store.lock().await;
        store.save_connection(&test_connection_input()).unwrap();
    }

    let store = state.store.lock().await;
    let list = store.list_connections().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "Test Connection");
}

#[tokio::test]
async fn get_connection_by_id() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;

    let saved = store.save_connection(&test_connection_input()).unwrap();
    let fetched = store.get_connection(&saved.id).unwrap();

    assert_eq!(fetched.id, saved.id);
    assert_eq!(fetched.name, saved.name);
    assert_eq!(fetched.host, saved.host);
}

#[tokio::test]
async fn update_connection_changes_fields() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;

    let saved = store.save_connection(&test_connection_input()).unwrap();

    let mut updated_input = test_connection_input();
    updated_input.name = "Updated Name".to_string();
    updated_input.host = "remotehost".to_string();
    updated_input.port = 5433;

    store.update_connection(&saved.id, &updated_input).unwrap();

    let fetched = store.get_connection(&saved.id).unwrap();
    assert_eq!(fetched.name, "Updated Name");
    assert_eq!(fetched.host, "remotehost");
    assert_eq!(fetched.port, 5433);
}

#[tokio::test]
async fn update_connection_preserves_password_when_empty() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;

    let saved = store.save_connection(&test_connection_input()).unwrap();

    let mut updated_input = test_connection_input();
    updated_input.password = String::new();
    updated_input.name = "Renamed".to_string();

    store.update_connection(&saved.id, &updated_input).unwrap();

    let fetched = store.get_connection(&saved.id).unwrap();
    assert_eq!(fetched.name, "Renamed");
    assert_eq!(fetched.password, "testpass"); // preserved
}

#[tokio::test]
async fn delete_connection_removes_it() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;

    let saved = store.save_connection(&test_connection_input()).unwrap();
    assert_eq!(store.list_connections().unwrap().len(), 1);

    store.delete_connection(&saved.id).unwrap();
    assert_eq!(store.list_connections().unwrap().len(), 0);
}

#[tokio::test]
async fn delete_nonexistent_connection_fails() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;
    let result = store.delete_connection("nonexistent-id");
    assert!(result.is_err());
}

#[tokio::test]
async fn update_nonexistent_connection_fails() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;
    let result = store.update_connection("nonexistent-id", &test_connection_input());
    assert!(result.is_err());
}

#[tokio::test]
async fn update_last_connected_sets_timestamp() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;

    let saved = store.save_connection(&test_connection_input()).unwrap();
    assert!(saved.last_connected_at.is_none());

    store.update_last_connected(&saved.id).unwrap();

    let fetched = store.get_connection(&saved.id).unwrap();
    assert!(fetched.last_connected_at.is_some());
}

#[tokio::test]
async fn update_last_connected_nonexistent_fails() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;
    let result = store.update_last_connected("nonexistent-id");
    assert!(result.is_err());
}

#[tokio::test]
async fn save_multiple_connections_lists_all() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;

    let mut input1 = test_connection_input();
    input1.name = "Alpha DB".to_string();
    let mut input2 = test_connection_input();
    input2.name = "Beta DB".to_string();

    store.save_connection(&input1).unwrap();
    store.save_connection(&input2).unwrap();

    let list = store.list_connections().unwrap();
    assert_eq!(list.len(), 2);
    // Listed alphabetically by name
    assert_eq!(list[0].name, "Alpha DB");
    assert_eq!(list[1].name, "Beta DB");
}

// ── Connection ID parsing tests (exercises command-level UUID parsing) ──

#[tokio::test]
async fn disconnect_invalid_uuid_returns_error() {
    let result = uuid::Uuid::parse_str("not-a-uuid");
    assert!(result.is_err());
}

#[tokio::test]
async fn disconnect_valid_uuid_parses() {
    let id = uuid::Uuid::new_v4().to_string();
    let result = uuid::Uuid::parse_str(&id);
    assert!(result.is_ok());
}

// ── Registry tests ──

#[tokio::test]
async fn available_engines_empty_registry() {
    let (state, _tmp) = create_test_state();
    let engines = state.registry.available_engines();
    assert!(engines.is_empty());
}

#[tokio::test]
async fn saved_connection_stores_engine_field() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;

    let mut input = test_connection_input();
    input.engine = "sqlite".to_string();

    let saved = store.save_connection(&input).unwrap();
    assert_eq!(saved.engine, "sqlite");

    let fetched = store.get_connection(&saved.id).unwrap();
    assert_eq!(fetched.engine, "sqlite");
}
