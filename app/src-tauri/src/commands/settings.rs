use agent_loaf_core::base::paths;
use agent_loaf_core::base::persist::save_json;
use agent_loaf_core::base::settings::Preferences;

use agent_loaf_core::base::messages::{self, Msg};
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;

use crate::base::sync::LockExt;
use crate::session::monitor::AppState;
use crate::session::notifier;
use crate::setup::installer;

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Preferences {
    state.settings.locked().preferences()
}

#[tauri::command]
pub fn save_settings(app: AppHandle, state: State<'_, AppState>, settings: Preferences) -> Result<(), String> {
    let was_enabled = state.settings.locked().launch_at_login;
    // 通知の設定だけを変えたときにログイン項目を書き直さない
    let changes_login = settings.launch_at_login != was_enabled;
    if changes_login {
        set_launch_at_login(&app, settings.launch_at_login)
            .map_err(|error| messages::format(state.lang, Msg::ErrorAutostart, &[("error", &error.to_string())]))?;
    }
    let mut current = state.settings.locked();
    let mut next = current.clone();
    next.apply_preferences(settings);
    if let Err(error) = save_json(&paths::settings_path(&state.home), &next) {
        // 保存できなかったときは画面が元の設定に戻るので、OS のログイン項目も元に戻して食い違わないようにする
        if changes_login {
            let _ = set_launch_at_login(&app, was_enabled);
        }
        return Err(messages::format(
            state.lang,
            Msg::ErrorSaveSettings,
            &[("error", &error.to_string())],
        ));
    }
    *current = next;
    Ok(())
}

fn set_launch_at_login(app: &AppHandle, enabled: bool) -> Result<(), tauri_plugin_autostart::Error> {
    let autostart = app.autolaunch();
    if enabled {
        autostart.enable()
    } else {
        autostart.disable()
    }
}

#[tauri::command]
pub fn open_notification_settings(app: AppHandle) {
    let url = format!(
        "x-apple.systempreferences:com.apple.Notifications-Settings.extension?id={}",
        app.config().identifier
    );
    if let Err(error) = std::process::Command::new("open").arg(url).spawn() {
        eprintln!("failed to open System Settings: {error}");
    }
}

#[tauri::command]
pub fn refresh_notification_access(app: AppHandle) {
    let handle = app.clone();
    notifier::refresh_authorization(move || installer::publish(&handle));
}
