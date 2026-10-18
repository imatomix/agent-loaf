use agent_loaf_core::base::paths;
use agent_loaf_core::base::persist::save_json;
use agent_loaf_core::session::order::move_before;
use agent_loaf_core::session::view::SessionView;

use agent_loaf_core::base::messages::{self, Msg};
use tauri::{AppHandle, State};

use crate::monitor::AppState;
use crate::sync::LockExt;

#[tauri::command]
pub fn get_sessions(state: State<'_, AppState>) -> Vec<SessionView> {
    crate::monitor::current_views(&state)
}

#[tauri::command]
pub fn move_session(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    before_id: Option<String>,
) -> Result<(), String> {
    {
        let mut settings = state.settings.locked();
        let mut updated = settings.clone();
        updated.session_order = move_before(&settings.session_order, &id, before_id.as_deref());
        save_json(&paths::settings_path(&state.home), &updated)
            .map_err(|error| messages::format(state.lang, Msg::ErrorSaveOrder, &[("error", &error.to_string())]))?;
        *settings = updated;
    }
    crate::monitor::publish(&app);
    Ok(())
}

// ps と open を呼ぶので、画面を固めないよう別のスレッドで動かす
#[tauri::command]
pub async fn jump_session(app: AppHandle, id: String) {
    let _ = tauri::async_runtime::spawn_blocking(move || {
        // 移動の前に一覧は閉じているので、詳細を出すのは画面ではなくこちらで行う
        if !crate::jumper::jump(&app, &id) {
            crate::tray::show_details(&app, &id);
        }
    })
    .await;
}

#[tauri::command]
pub fn take_pending_details(state: State<'_, AppState>) -> Option<String> {
    state.pending_details.locked().take()
}
