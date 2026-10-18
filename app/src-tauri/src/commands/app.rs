use agent_loaf_core::base::messages;
use tauri::{AppHandle, State};

use crate::session::monitor::AppState;

#[tauri::command]
pub fn get_messages(state: State<'_, AppState>) -> std::collections::HashMap<String, String> {
    messages::table(state.lang).clone()
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}
