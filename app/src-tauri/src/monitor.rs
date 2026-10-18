use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use agent_loaf_core::base::messages::Lang;
use agent_loaf_core::base::paths;
use agent_loaf_core::base::persist::{load_json, save_json};
use agent_loaf_core::base::settings::Settings;
use agent_loaf_core::process::ps::{process_cwd, run_ps, tracked_process_rows};
use agent_loaf_core::session::notify::{notice_for, Notice};
use agent_loaf_core::session::spool::{drain_spool, MAX_SPOOL_FILES};
use agent_loaf_core::session::store::{NotificationIds, SessionStore, INTERRUPT_QUIET_MS};
use agent_loaf_core::session::transcript::transcript_ends_with_interrupt;
use agent_loaf_core::session::view::{session_views, SessionView};
use agent_loaf_core::setup::config_dirs::Monitored;
use agent_loaf_core::setup::install::install_hook_binary;
use tauri::{AppHandle, Emitter, Manager};

use crate::notifier;
use crate::sync::LockExt;
use crate::tray::TRAY_ID;
use crate::tray_icon::{tray_icon, IconCache};

// アプリ停止中に溜まった古いイベントで通知を乱発しないため、この時間より古い変化は通知しない
pub const NOTIFY_FRESHNESS_MS: u64 = 30_000;
const TICK: Duration = Duration::from_secs(1);
const PROCESS_CHECK_EVERY_TICKS: u64 = 3;

pub struct AppState {
    pub home: PathBuf,
    pub store: Mutex<SessionStore>,
    pub settings: Mutex<Settings>,
    pub list_hidden_at_ms: AtomicU64,
    pub suppress_hide: AtomicBool,
    pub list_open: AtomicBool,
    // 起動した直後などは画面の読み込みが終わっていないことがあるので、「開いたらこの行の詳細を出す」は画面ではなくここで預かる
    pub pending_details: Mutex<Option<String>>,
    pub lang: Lang,
    pub tray_icon: Mutex<IconCache>,
}

pub fn should_notify(change_ts_ms: u64, now_ms: u64) -> bool {
    now_ms.saturating_sub(change_ts_ms) <= NOTIFY_FRESHNESS_MS
}

pub fn load_state(home: PathBuf) -> AppState {
    let mut store: SessionStore = load_json(&paths::state_path(&home));
    store.forget_unknown_processes();
    AppState {
        store: Mutex::new(store),
        settings: Mutex::new(load_json(&paths::settings_path(&home))),
        list_hidden_at_ms: AtomicU64::new(0),
        suppress_hide: AtomicBool::new(false),
        list_open: AtomicBool::new(false),
        pending_details: Mutex::new(None),
        lang: crate::lang::detect(),
        tray_icon: Mutex::new(IconCache::default()),
        home,
    }
}

pub fn install_bundled_hook(home: &Path) {
    let Some(source) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("agent-loaf-hook")))
    else {
        return;
    };
    if let Err(error) = install_hook_binary(&source, &paths::bin_path(home)) {
        eprintln!("failed to install agent-loaf-hook: {error}");
    }
}

pub fn spawn(app: AppHandle) {
    thread::spawn(move || {
        let mut tick: u64 = 0;
        loop {
            // 1 回分の処理で panic しても、次の回からは動き続ける。ロックは LockExt で壊れても使える
            let changed = catch_unwind(AssertUnwindSafe(|| {
                run_tick(&app, tick.is_multiple_of(PROCESS_CHECK_EVERY_TICKS))
            }))
            .unwrap_or(false);
            if changed {
                publish(&app);
            }
            crate::updater::tick(&app, paths::now_ms());
            tick = tick.wrapping_add(1);
            thread::sleep(TICK);
        }
    });
}

fn run_tick(app: &AppHandle, check_processes: bool) -> bool {
    let state = app.state::<AppState>();
    let now = paths::now_ms();
    let batch = drain_spool(&paths::events_dir(&state.home), MAX_SPOOL_FILES);
    if batch.skipped > 0 {
        eprintln!("dropped {} unreadable or excess event files", batch.skipped);
    }
    let (notify_on, monitored) = {
        let settings = state.settings.locked();
        (settings.notify_on.clone(), monitored_dirs(&settings))
    };
    let mut notices: Vec<Notice> = Vec::new();
    let deliveries: Vec<(Notice, NotificationIds)>;
    let mut changed = false;
    {
        let mut store = state.store.locked();
        for event in &batch.events {
            if let Some(change) = store.apply(event) {
                changed = true;
                if should_notify(change.ts_ms, now) && monitored.includes(&change.session_id, &change.config_dir) {
                    notices.extend(notice_for(&change, &notify_on, state.lang));
                }
            }
        }
        if check_processes {
            changed |= reconcile_processes(&mut store, now, state.list_open.load(Ordering::Relaxed));
            changed |= store.detect_interrupts(now, INTERRUPT_QUIET_MS, transcript_ends_with_interrupt);
        }
        if state.list_open.load(Ordering::Relaxed) {
            changed |= store.mark_finished_seen(now);
        }
        deliveries = notices
            .into_iter()
            .map(|notice| {
                let ids = store.record_notification(&notice.session_id, now);
                (notice, ids)
            })
            .collect();
        changed |= !deliveries.is_empty();
        changed |= store.hide_seen_finished(now);
        changed |= store.prune(now);
        if changed {
            if let Err(error) = save_json(&paths::state_path(&state.home), &*store) {
                eprintln!("failed to save state: {error}");
            }
        }
    }
    for (notice, ids) in deliveries {
        notifier::show(&notice.title, &notice.body, &ids);
    }
    changed
}

// 全プロセスを読むのは状態不明の行を見つけるためだけで、それは一覧にしか出ない。閉じている間は知っている pid が生きているかだけを見る
fn reconcile_processes(store: &mut SessionStore, now: u64, list_open: bool) -> bool {
    let rows = if list_open {
        run_ps()
    } else {
        tracked_process_rows(&store.tracked_pids())
    };
    rows.is_some_and(|rows| store.reconcile(&rows, now, process_cwd))
}

pub fn check_processes_now(app: &AppHandle) {
    let state = app.state::<AppState>();
    let changed = reconcile_processes(&mut state.store.locked(), paths::now_ms(), true);
    if changed {
        publish(app);
    }
}

fn monitored_dirs(settings: &Settings) -> Monitored {
    Monitored::new(settings.config_dirs.as_deref().unwrap_or_default())
}

// ロックは必ず store → settings の順に取る。逆の順で取る箇所を作るとデッドロックする
pub fn current_views(state: &AppState) -> Vec<SessionView> {
    let store = state.store.locked();
    let mut settings = state.settings.locked();
    let (views, next_order) = session_views(
        store.sessions(),
        &settings.session_order,
        &monitored_dirs(&settings),
        state.lang,
    );
    if next_order != settings.session_order {
        settings.session_order = next_order;
        if let Err(error) = save_json(&paths::settings_path(&state.home), &*settings) {
            eprintln!("failed to save session order: {error}");
        }
    }
    views
}

pub fn publish(app: &AppHandle) {
    let state = app.state::<AppState>();
    let views = current_views(&state);
    let monitored = monitored_dirs(&state.settings.locked());
    let tray_status = state
        .store
        .locked()
        .tray_status(|s| monitored.includes(&s.id, &s.config_dir));
    let _ = app.emit("sessions-updated", &views);
    if state.tray_icon.locked().needs_update(tray_status) {
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            // set_icon だけだとテンプレート指定が外れ、明るいメニューバーで黒く表示されなくなる
            let _ = tray.set_icon_with_as_template(Some(tray_icon(tray_status)), true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notifies_only_fresh_changes() {
        assert!(should_notify(100_000, 100_000));
        assert!(should_notify(100_000 - NOTIFY_FRESHNESS_MS, 100_000));
        assert!(!should_notify(100_000 - NOTIFY_FRESHNESS_MS - 1, 100_000));
        assert!(should_notify(100_500, 100_000), "時計のずれで未来の時刻でも通知する");
    }
}
