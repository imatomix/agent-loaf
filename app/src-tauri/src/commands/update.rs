use tauri::AppHandle;

#[tauri::command]
pub fn get_update(app: AppHandle) -> agent_loaf_core::update::UpdateState {
    crate::updater::current(&app)
}

// 手元で作ったアプリと、配っているアプリを、版の表示で見分けられるようにする
#[tauri::command]
pub fn is_release_build() -> bool {
    crate::updater::ENABLED
}

#[tauri::command]
pub fn install_update(app: AppHandle) {
    crate::updater::install(app);
}
