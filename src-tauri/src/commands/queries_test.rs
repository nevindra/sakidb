use super::mock_helpers::*;

// ── Saved queries tests ──

#[tokio::test]
async fn save_query_returns_saved_query() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;

    let result = store.save_query("My Query", "SELECT 1", Some("conn-1"), Some("mydb"));
    assert!(result.is_ok());

    let saved = result.unwrap();
    assert_eq!(saved.name, "My Query");
    assert_eq!(saved.sql, "SELECT 1");
    assert_eq!(saved.connection_id, Some("conn-1".to_string()));
    assert_eq!(saved.database_name, Some("mydb".to_string()));
    assert!(!saved.id.is_empty());
}

#[tokio::test]
async fn save_query_without_connection() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;

    let saved = store.save_query("Standalone", "SELECT 42", None, None).unwrap();
    assert!(saved.connection_id.is_none());
    assert!(saved.database_name.is_none());
}

#[tokio::test]
async fn list_saved_queries_empty() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;
    let list = store.list_saved_queries().unwrap();
    assert!(list.is_empty());
}

#[tokio::test]
async fn list_saved_queries_returns_all() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;

    store.save_query("Q1", "SELECT 1", None, None).unwrap();
    store.save_query("Q2", "SELECT 2", None, None).unwrap();

    let list = store.list_saved_queries().unwrap();
    assert_eq!(list.len(), 2);
}

#[tokio::test]
async fn update_saved_query_name() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;

    let saved = store.save_query("Original", "SELECT 1", None, None).unwrap();
    let updated = store
        .update_saved_query(&saved.id, Some("Renamed"), None)
        .unwrap();

    assert_eq!(updated.name, "Renamed");
    assert_eq!(updated.sql, "SELECT 1"); // unchanged
}

#[tokio::test]
async fn update_saved_query_sql() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;

    let saved = store.save_query("Q", "SELECT 1", None, None).unwrap();
    let updated = store
        .update_saved_query(&saved.id, None, Some("SELECT 2"))
        .unwrap();

    assert_eq!(updated.name, "Q"); // unchanged
    assert_eq!(updated.sql, "SELECT 2");
}

#[tokio::test]
async fn delete_saved_query_removes_it() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;

    let saved = store.save_query("Q", "SELECT 1", None, None).unwrap();
    store.delete_saved_query(&saved.id).unwrap();

    let list = store.list_saved_queries().unwrap();
    assert!(list.is_empty());
}

#[tokio::test]
async fn delete_nonexistent_saved_query_fails() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;
    let result = store.delete_saved_query("nonexistent-id");
    assert!(result.is_err());
}

// ── Query history tests ──

#[tokio::test]
async fn add_query_history_returns_entry() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;

    let entry = store
        .add_query_history("SELECT 1", Some("c1"), Some("db1"), Some(10), Some(1))
        .unwrap();

    assert_eq!(entry.sql, "SELECT 1");
    assert_eq!(entry.connection_id, Some("c1".to_string()));
    assert_eq!(entry.database_name, Some("db1".to_string()));
    assert_eq!(entry.execution_time_ms, Some(10));
    assert_eq!(entry.row_count, Some(1));
}

#[tokio::test]
async fn query_history_deduplicates() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;

    let e1 = store
        .add_query_history("SELECT 1", Some("c1"), Some("db1"), Some(10), Some(1))
        .unwrap();
    let e2 = store
        .add_query_history("SELECT 1", Some("c1"), Some("db1"), Some(20), Some(2))
        .unwrap();

    // Same entry is updated, not duplicated
    assert_eq!(e1.id, e2.id);
    assert_eq!(e2.execution_time_ms, Some(20));
}

#[tokio::test]
async fn list_query_history_returns_entries() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;

    store
        .add_query_history("SELECT 1", None, None, None, None)
        .unwrap();
    store
        .add_query_history("SELECT 2", None, None, None, None)
        .unwrap();

    let list = store.list_query_history(None).unwrap();
    assert_eq!(list.len(), 2);
}

#[tokio::test]
async fn list_query_history_respects_limit() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;

    for i in 0..5 {
        store
            .add_query_history(&format!("SELECT {}", i), None, None, None, None)
            .unwrap();
    }

    let list = store.list_query_history(Some(3)).unwrap();
    assert_eq!(list.len(), 3);
}

#[tokio::test]
async fn clear_query_history_removes_all() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;

    store
        .add_query_history("SELECT 1", None, None, None, None)
        .unwrap();
    store
        .add_query_history("SELECT 2", None, None, None, None)
        .unwrap();

    store.clear_query_history().unwrap();
    let list = store.list_query_history(None).unwrap();
    assert!(list.is_empty());
}

#[tokio::test]
async fn save_from_history_creates_saved_query() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;

    let entry = store
        .add_query_history("SELECT * FROM users", Some("c1"), Some("db1"), Some(50), Some(10))
        .unwrap();

    let saved = store
        .save_from_history(&entry.id, "Users query")
        .unwrap();

    assert_eq!(saved.name, "Users query");
    assert_eq!(saved.sql, "SELECT * FROM users");
    assert_eq!(saved.connection_id, Some("c1".to_string()));
    assert_eq!(saved.database_name, Some("db1".to_string()));
}

#[tokio::test]
async fn save_from_nonexistent_history_fails() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;
    let result = store.save_from_history("nonexistent-id", "Name");
    assert!(result.is_err());
}
