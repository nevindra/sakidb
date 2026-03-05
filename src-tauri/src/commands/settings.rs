use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub async fn get_keybinding_overrides(
    state: State<'_, AppState>,
) -> Result<Vec<(String, Option<String>)>, String> {
    let store = state.store.lock().await;
    store.get_keybinding_overrides().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_keybinding(
    state: State<'_, AppState>,
    command_id: String,
    keybinding: Option<String>,
) -> Result<(), String> {
    let store = state.store.lock().await;
    store
        .set_keybinding(&command_id, keybinding.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reset_keybinding(
    state: State<'_, AppState>,
    command_id: String,
) -> Result<(), String> {
    let store = state.store.lock().await;
    store.reset_keybinding(&command_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reset_all_keybindings(
    state: State<'_, AppState>,
) -> Result<(), String> {
    let store = state.store.lock().await;
    store.reset_all_keybindings().map_err(|e| e.to_string())
}
