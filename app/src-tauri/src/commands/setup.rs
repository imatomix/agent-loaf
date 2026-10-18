use agent_loaf_core::base::paths;
use agent_loaf_core::base::persist::save_json;
use std::path::Path;
use std::sync::atomic::Ordering;

use agent_loaf_core::base::messages::{self, Msg};
use agent_loaf_core::setup::claude_cli::{is_executable, run_command};
use agent_loaf_core::setup::codex_plugin::trust_command_script;
use agent_loaf_core::setup::config_dirs::is_monitored_codex_dir;
use std::time::Duration;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::installer;
use crate::monitor::AppState;
use crate::sync::LockExt;

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
