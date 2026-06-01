use crate::db::Store;
use crate::models::ConnectionInput;

fn test_input() -> ConnectionInput {
    ConnectionInput {
        name: "Test DB".into(),
        engine: "postgres".into(),
        host: "localhost".into(),
        port: 5432,
        database: "testdb".into(),
        username: "user".into(),
        password: "secret123".into(),
        ssl_mode: "prefer".into(),
        options: std::collections::HashMap::new(),
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
fn keybindings_crud() {
    let store = Store::open_in_memory().unwrap();

    // Initially empty
    assert_eq!(store.get_keybinding_overrides().unwrap().len(), 0);

    // Set a keybinding
    store.set_keybinding("nav.new-query", Some("Ctrl+N")).unwrap();
    let overrides = store.get_keybinding_overrides().unwrap();
    assert_eq!(overrides.len(), 1);
    assert_eq!(overrides[0], ("nav.new-query".to_string(), Some("Ctrl+N".to_string())));

    // Unbind (set to None)
    store.set_keybinding("nav.new-query", None).unwrap();
    let overrides = store.get_keybinding_overrides().unwrap();
    assert_eq!(overrides[0].1, None);

    // Reset single
    store.reset_keybinding("nav.new-query").unwrap();
    assert_eq!(store.get_keybinding_overrides().unwrap().len(), 0);

    // Reset all
    store.set_keybinding("a", Some("Ctrl+A")).unwrap();
    store.set_keybinding("b", Some("Ctrl+B")).unwrap();
    store.reset_all_keybindings().unwrap();
    assert_eq!(store.get_keybinding_overrides().unwrap().len(), 0);
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
