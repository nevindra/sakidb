use super::mock_helpers::*;

#[tokio::test]
async fn get_keybinding_overrides_initially_empty() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;
    let overrides = store.get_keybinding_overrides().unwrap();
    assert!(overrides.is_empty());
}

#[tokio::test]
async fn set_keybinding_creates_override() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;

    store.set_keybinding("nav.new-query", Some("Ctrl+N")).unwrap();

    let overrides = store.get_keybinding_overrides().unwrap();
    assert_eq!(overrides.len(), 1);
    assert_eq!(overrides[0].0, "nav.new-query");
    assert_eq!(overrides[0].1, Some("Ctrl+N".to_string()));
}

#[tokio::test]
async fn set_keybinding_unbind_sets_none() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;

    store.set_keybinding("nav.new-query", Some("Ctrl+N")).unwrap();
    store.set_keybinding("nav.new-query", None).unwrap();

    let overrides = store.get_keybinding_overrides().unwrap();
    assert_eq!(overrides.len(), 1);
    assert_eq!(overrides[0].1, None);
}

#[tokio::test]
async fn set_keybinding_updates_existing() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;

    store.set_keybinding("nav.new-query", Some("Ctrl+N")).unwrap();
    store.set_keybinding("nav.new-query", Some("Ctrl+Shift+N")).unwrap();

    let overrides = store.get_keybinding_overrides().unwrap();
    assert_eq!(overrides.len(), 1);
    assert_eq!(overrides[0].1, Some("Ctrl+Shift+N".to_string()));
}

#[tokio::test]
async fn reset_keybinding_removes_override() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;

    store.set_keybinding("nav.new-query", Some("Ctrl+N")).unwrap();
    store.reset_keybinding("nav.new-query").unwrap();

    let overrides = store.get_keybinding_overrides().unwrap();
    assert!(overrides.is_empty());
}

#[tokio::test]
async fn reset_nonexistent_keybinding_is_noop() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;

    // Resetting a command that was never set should not error
    let result = store.reset_keybinding("nonexistent.command");
    assert!(result.is_ok());
}

#[tokio::test]
async fn reset_all_keybindings_clears_all() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;

    store.set_keybinding("cmd.a", Some("Ctrl+A")).unwrap();
    store.set_keybinding("cmd.b", Some("Ctrl+B")).unwrap();
    store.set_keybinding("cmd.c", Some("Ctrl+C")).unwrap();

    store.reset_all_keybindings().unwrap();

    let overrides = store.get_keybinding_overrides().unwrap();
    assert!(overrides.is_empty());
}

#[tokio::test]
async fn multiple_keybindings_coexist() {
    let (state, _tmp) = create_test_state();
    let store = state.store.lock().await;

    store.set_keybinding("cmd.a", Some("Ctrl+A")).unwrap();
    store.set_keybinding("cmd.b", Some("Ctrl+B")).unwrap();

    let overrides = store.get_keybinding_overrides().unwrap();
    assert_eq!(overrides.len(), 2);
}
