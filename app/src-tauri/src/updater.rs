use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use agent_loaf_core::base::paths;
use agent_loaf_core::update::{is_due, UpdateEvent, UpdateState};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::sync::LockExt;

// 作るときに AGENT_LOAF_RELEASE を渡した公開のビルドだけが確かめる。開発のビルドは更新の設定（鍵）がなくても動かす
pub const ENABLED: bool = option_env!("AGENT_LOAF_RELEASE").is_some();
// 通信が途中で止まると、確かめる処理もダウンロードも終わらず「ダウンロードしています」のまま固まる。
// ダウンロードの全体にもかかる制限なので、遅い回線でも十数 MB を落とせる長さにする
const NETWORK_TIMEOUT: Duration = Duration::from_secs(10 * 60);

pub struct UpdaterState {
    started_ms: u64,
    last_check_ms: Mutex<Option<u64>>,
    last_check_failed: AtomicBool,
    // 確かめる処理とダウンロードは別々に印を持ち、片方が長引いても、もう片方を塞がない
    checking: AtomicBool,
    installing: AtomicBool,
    state: Mutex<UpdateState>,
    found: Mutex<Option<Update>>,
}

impl Default for UpdaterState {
    fn default() -> Self {
        UpdaterState {
            started_ms: paths::now_ms(),
            last_check_ms: Mutex::new(None),
            last_check_failed: AtomicBool::new(false),
            checking: AtomicBool::new(false),
            installing: AtomicBool::new(false),
            state: Mutex::default(),
            found: Mutex::new(None),
        }
    }
}

pub fn current(app: &AppHandle) -> UpdateState {
    app.state::<UpdaterState>().state.locked().clone()
}

fn apply(app: &AppHandle, event: UpdateEvent) {
    let updater = app.state::<UpdaterState>();
    let next = {
        let mut state = updater.state.locked();
        let next = state.next(&event);
        if next == *state {
            return;
        }
        *state = next.clone();
        next
    };
    let _ = app.emit("update-updated", next);
}

// 監視の 1 秒ごとの処理から呼ぶ。確かめる時刻でなければ何もしないので、ふだんの負荷は時刻の比較だけ
pub fn tick(app: &AppHandle, now_ms: u64) {
    if !ENABLED {
        return;
    }
    let updater = app.state::<UpdaterState>();
    let failed = updater.last_check_failed.load(Ordering::Relaxed);
    if !is_due(now_ms, updater.started_ms, *updater.last_check_ms.locked(), failed) {
        return;
    }
    *updater.last_check_ms.locked() = Some(now_ms);
    if updater.checking.swap(true, Ordering::Relaxed) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let event = match check(&app).await {
            Ok(Some(update)) => {
                let version = update.version.clone();
                *app.state::<UpdaterState>().found.locked() = Some(update);
                UpdateEvent::Found(version)
            }
            Ok(None) => UpdateEvent::NotFound,
            Err(error) => {
                eprintln!("update check failed: {error}");
                UpdateEvent::CheckFailed
            }
        };
        let updater = app.state::<UpdaterState>();
        updater
            .last_check_failed
            .store(event == UpdateEvent::CheckFailed, Ordering::Relaxed);
        updater.checking.store(false, Ordering::Relaxed);
        apply(&app, event);
    });
}

async fn check(app: &AppHandle) -> tauri_plugin_updater::Result<Option<Update>> {
    app.updater_builder().timeout(NETWORK_TIMEOUT).build()?.check().await
}

pub fn install(app: AppHandle) {
    let updater = app.state::<UpdaterState>();
    // 続けて押されても、ダウンロードは 1 つだけにする
    if updater.installing.swap(true, Ordering::Relaxed) {
        return;
    }
    let Some(update) = updater.found.locked().clone() else {
        updater.installing.store(false, Ordering::Relaxed);
        return;
    };
    apply(&app, UpdateEvent::Started);
    tauri::async_runtime::spawn(async move {
        let mut downloaded: u64 = 0;
        let progress_app = app.clone();
        let result = update
            .download_and_install(
                move |chunk, total| {
                    downloaded += chunk as u64;
                    apply(&progress_app, UpdateEvent::Progress { downloaded, total });
                },
                || {},
            )
            .await;
        match result {
            // 入れ替えが済んだら、新しい版で起動し直す。hook とプラグインは起動したときの処理で新しくなる
            Ok(()) => app.restart(),
            Err(error) => {
                eprintln!("update install failed: {error}");
                app.state::<UpdaterState>().installing.store(false, Ordering::Relaxed);
                apply(&app, UpdateEvent::InstallFailed);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn development_builds_never_check() {
        // cargo test は AGENT_LOAF_RELEASE を付けずに作るので、確かめる処理は止まっている
        assert!(!ENABLED);
    }
}
