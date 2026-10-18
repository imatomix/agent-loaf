use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use agent_loaf_core::process::jump::{jump_target, JumpTarget};
use agent_loaf_core::process::procinfo::ancestor_rows;
use agent_loaf_core::setup::claude_cli::run_command;
use tauri::{AppHandle, Manager};

use crate::monitor::AppState;
use crate::sync::LockExt;

const OPEN_TIMEOUT: Duration = Duration::from_secs(5);

// 行や通知を続けて押すと open が重ねて動き、ウィンドウが行き来する
static JUMPING: AtomicBool = AtomicBool::new(false);

// 移動の途中で panic しても、印が立ったままにならないよう、抜けるときに必ず下ろす
struct InProgress<'a>(&'a AtomicBool);

impl<'a> InProgress<'a> {
    fn start(flag: &'a AtomicBool) -> Option<Self> {
        (!flag.swap(true, Ordering::Relaxed)).then_some(InProgress(flag))
    }
}

impl Drop for InProgress<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Relaxed);
    }
}

pub fn jump(app: &AppHandle, session_id: &str) -> bool {
    // 前の移動がまだ終わっていない。詳細を開かせないよう、移動したものとして扱う
    let Some(_jumping) = InProgress::start(&JUMPING) else {
        return true;
    };
    jump_once(app, session_id)
}

fn jump_once(app: &AppHandle, session_id: &str) -> bool {
    let session = app
        .state::<AppState>()
        .store
        .locked()
        .sessions()
        .iter()
        .find(|s| s.id == session_id)
        .cloned();
    let Some(session) = session else { return false };
    let Some(pid) = session.claude_pid else { return false };
    let rows = ancestor_rows(pid);
    let JumpTarget::App { app: path, folder } = jump_target(&session, &rows) else {
        return false;
    };
    crate::tray::hide_list(app);
    // フォルダーがもうないと open は失敗するので、そのときはアプリだけを前に出す
    folder.as_deref().is_some_and(|folder| open_app(&path, Some(folder))) || open_app(&path, None)
}

fn open_app(app: &Path, folder: Option<&str>) -> bool {
    let app = app.to_string_lossy();
    let mut args = vec!["-a", app.as_ref()];
    args.extend(folder);
    open(&args)
}

fn open(args: &[&str]) -> bool {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    match run_command(Path::new("/usr/bin/open"), &args, &[], OPEN_TIMEOUT) {
        Ok(_) => true,
        Err(error) => {
            eprintln!("open failed: {error:?}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_jump_runs_at_a_time_and_a_panic_releases_it() {
        let flag = AtomicBool::new(false);
        let first = InProgress::start(&flag).expect("最初は始められる");
        assert!(InProgress::start(&flag).is_none(), "終わるまでは次を始めない");
        drop(first);
        assert!(InProgress::start(&flag).is_some());
        let result = std::panic::catch_unwind(|| {
            let _guard = InProgress::start(&flag).unwrap();
            panic!("移動の途中で落ちた");
        });
        assert!(result.is_err());
        assert!(InProgress::start(&flag).is_some(), "panic の後も移動を受け付ける");
    }
}
