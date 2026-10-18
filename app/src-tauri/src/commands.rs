use agent_loaf_core::order::move_before;
use agent_loaf_core::paths;
use agent_loaf_core::persist::save_json;
use agent_loaf_core::settings::Preferences;
use agent_loaf_core::view::SessionView;
use std::path::Path;
use std::sync::atomic::Ordering;

use agent_loaf_core::claude_cli::{is_executable, run_command};
use agent_loaf_core::codex_plugin::trust_command_script;
use agent_loaf_core::config_dirs::is_monitored_codex_dir;
use agent_loaf_core::messages::{self, Msg};
use std::time::Duration;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_dialog::DialogExt;

use crate::installer;
use crate::monitor::AppState;
use crate::notifier;
use crate::sync::LockExt;

#[tauri::command]
pub fn get_sessions(state: State<'_, AppState>) -> Vec<SessionView> {
    crate::monitor::current_views(&state)
}

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

#[tauri::command]
pub fn get_setup(app: AppHandle) -> installer::SetupView {
    installer::view(&app)
}

#[tauri::command]
pub fn add_config_dir(app: AppHandle) {
    app.state::<AppState>().suppress_hide.store(true, Ordering::Relaxed);
    let title = messages::text(app.state::<AppState>().lang, Msg::DialogChooseConfigDir);
    let mut dialog = app.dialog().file().set_title(title);
    if let Some(window) = app.get_webview_window("list") {
        dialog = dialog.set_parent(&window);
    }
    let handle = app.clone();
    dialog.pick_folder(move |folder| {
        handle.state::<AppState>().suppress_hide.store(false, Ordering::Relaxed);
        if let Some(path) = folder.and_then(|folder| folder.into_path().ok()) {
            installer::send(&handle, installer::Job::Add(path.to_string_lossy().into_owned()));
        }
    });
}

#[tauri::command]
pub fn retry_config_dir(app: AppHandle, path: String) {
    installer::send(&app, installer::Job::Retry(path));
}

#[tauri::command]
pub fn remove_config_dir(app: AppHandle, path: String) {
    installer::send(&app, installer::Job::Remove(path));
}

#[tauri::command]
pub fn set_claude_path(app: AppHandle, state: State<'_, AppState>, path: String) -> Result<(), String> {
    let trimmed = path.trim();
    if !is_executable(Path::new(trimmed)) {
        return Err(messages::text(state.lang, Msg::ErrorClaudePathInvalid));
    }
    {
        let mut settings = state.settings.locked();
        let mut updated = settings.clone();
        updated.claude_path = Some(trimmed.to_string());
        save_json(&paths::settings_path(&state.home), &updated)
            .map_err(|error| messages::format(state.lang, Msg::ErrorSaveSettings, &[("error", &error.to_string())]))?;
        *settings = updated;
    }
    installer::send(&app, installer::Job::Refresh);
    Ok(())
}

#[tauri::command]
pub fn set_codex_path(app: AppHandle, state: State<'_, AppState>, path: String) -> Result<(), String> {
    let trimmed = path.trim();
    if !is_executable(Path::new(trimmed)) {
        return Err(messages::text(state.lang, Msg::ErrorCodexPathInvalid));
    }
    {
        let mut settings = state.settings.locked();
        let mut updated = settings.clone();
        updated.codex_path = Some(trimmed.to_string());
        save_json(&paths::settings_path(&state.home), &updated)
            .map_err(|error| messages::format(state.lang, Msg::ErrorSaveSettings, &[("error", &error.to_string())]))?;
        *settings = updated;
    }
    installer::send(&app, installer::Job::Refresh);
    Ok(())
}

#[tauri::command]
pub fn open_codex_trust(app: AppHandle, state: State<'_, AppState>, path: String) -> Result<(), String> {
    let monitored = state.settings.locked().config_dirs.clone().unwrap_or_default();
    if !is_monitored_codex_dir(&monitored, &path) {
        return Err(messages::format(
            state.lang,
            Msg::ErrorNotMonitoredCodexDir,
            &[("dir", &installer::display(&path))],
        ));
    }
    let codex = installer::codex_path(&app).ok_or_else(|| messages::text(state.lang, Msg::ErrorCodexNotFound))?;
    let work_dir = state.home.join("codex-trust");
    let script = state.home.join("codex-trust.command");
    let fail = |error: std::io::Error| {
        messages::format(state.lang, Msg::ErrorCodexTrustFailed, &[("error", &error.to_string())])
    };
    paths::prepare_private_dir(&work_dir).map_err(fail)?;
    paths::write_private(
        &script,
        trust_command_script(&codex, Path::new(&path), &work_dir).as_bytes(),
    )
    .map_err(fail)?;
    // .command は実行できる権限がないと Terminal.app が開かない
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).map_err(fail)?;
    }
    run_command(
        Path::new("/usr/bin/open"),
        &[script.to_string_lossy().into_owned()],
        &[],
        Duration::from_secs(5),
    )
    .map(|_| ())
    .map_err(|error| {
        messages::format(
            state.lang,
            Msg::ErrorCodexTrustFailed,
            &[("error", &error.message(state.lang))],
        )
    })
}

#[tauri::command]
pub fn refresh_setup(app: AppHandle) {
    installer::send(&app, installer::Job::Refresh);
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

#[tauri::command]
pub fn get_messages(state: State<'_, AppState>) -> std::collections::HashMap<String, String> {
    messages::table(state.lang).clone()
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

#[tauri::command]
pub fn get_update(app: AppHandle) -> agent_loaf_core::update::UpdateState {
    crate::updater::current(&app)
}

#[tauri::command]
pub fn install_update(app: AppHandle) {
    crate::updater::install(app);
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}
