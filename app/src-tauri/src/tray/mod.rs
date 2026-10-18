pub mod icon;

use std::sync::atomic::Ordering;

use agent_loaf_core::base::paths;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WindowEvent};
use tauri_plugin_positioner::{Position, WindowExt};

use crate::base::sync::LockExt;
use crate::session::monitor::AppState;
use crate::tray::icon::tray_icon;

pub const TRAY_ID: &str = "main";
// 一覧が開いているときにアイコンを押すと、フォーカス喪失で閉じた直後にクリックで再び開いてしまうため
const REOPEN_GUARD_MS: u64 = 300;

pub fn setup_tray(app: &App) -> tauri::Result<()> {
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(tray_icon(None))
        .icon_as_template(true)
        .on_tray_icon_event(|tray, event| {
            tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
            if let TrayIconEvent::Click {
                button, button_state, ..
            } = event
            {
                if opens_list(button, button_state) {
                    toggle_list(tray.app_handle());
                }
            }
        })
        .build(app)?;
    Ok(())
}

fn opens_list(button: MouseButton, state: MouseButtonState) -> bool {
    button == MouseButton::Left && state == MouseButtonState::Down
}

pub fn setup_windows(app: &App) {
    if let Some(list) = app.get_webview_window("list") {
        watch_focus(app.handle(), &list);
    }
}

fn watch_focus(app: &AppHandle, list: &tauri::WebviewWindow) {
    let handle = app.clone();
    let window = list.clone();
    list.on_window_event(move |event| {
        if let WindowEvent::Focused(false) = event {
            let state = handle.state::<AppState>();
            // フォルダーを選ぶ画面やアラートを開くとフォーカスが移るが、そのときに閉じると設定画面が消えてしまう
            if state.suppress_hide.load(Ordering::Relaxed) {
                return;
            }
            close_window(&handle, &window);
        }
    });
}

// 閉じても画面（WebKit）は捨てずに隠す。捨てると開くたびに WebKit のプロセスを立ち上げ直すことになり、
// 長く使わなかった後やスリープの後は数秒かかる（2026-10-13 に測った値: 約 6 時間ぶりで 4.7 秒）。
// 隠しておく分のメモリーは約 32MB（描画のメモリーは隠してから 1 分ほどで返される）
fn close_window(app: &AppHandle, window: &tauri::WebviewWindow) {
    let _ = window.hide();
    let state = app.state::<AppState>();
    state.list_hidden_at_ms.store(paths::now_ms(), Ordering::Relaxed);
    // 読み込みの途中で閉じると預けた詳細が受け取られずに残り、次にふつうに開いたときに不意に開いてしまう
    state.pending_details.locked().take();
    set_list_open(app, false);
}

fn list_window(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    if let Some(window) = app.get_webview_window("list") {
        return Some(window);
    }
    let config = app.config().app.windows.iter().find(|w| w.label == "list")?.clone();
    let window = tauri::WebviewWindowBuilder::from_config(app, &config)
        .ok()?
        .build()
        .ok()?;
    watch_focus(app, &window);
    Some(window)
}

fn toggle_list(app: &AppHandle) {
    if let Some(window) = app
        .get_webview_window("list")
        .filter(|w| w.is_visible().unwrap_or(false))
    {
        close_window(app, &window);
        return;
    }
    let hidden_at = app.state::<AppState>().list_hidden_at_ms.load(Ordering::Relaxed);
    if paths::now_ms().saturating_sub(hidden_at) < REOPEN_GUARD_MS {
        return;
    }
    show_list(app);
}

pub fn hide_list(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("list") {
        close_window(app, &window);
    }
}

pub fn show_list(app: &AppHandle) {
    let Some(window) = list_window(app) else { return };
    if window.move_window(Position::TrayBottomCenter).is_err() {
        place_below_tray(app, &window);
    }
    let _ = window.show();
    let _ = window.set_focus();
    set_list_open(app, true);
    // 開いた直後に状態不明の行を出すため、全プロセスを 1 回読む。メインスレッドを止めないよう別のスレッドで
    let handle = app.clone();
    std::thread::spawn(move || crate::session::monitor::check_processes_now(&handle));
}

// 移動できなかったときは、一覧を開いてその行の詳細を出す。画面がまだ読み込み中なら、読み込みの後に受け取る
pub fn show_details(app: &AppHandle, session_id: &str) {
    *app.state::<AppState>().pending_details.locked() = Some(session_id.to_string());
    show_list(app);
    let _ = app.emit("show-details", ());
}

// 位置を決める部品は、アイコンの上でマウスが動くまでアイコンの位置を知らない。通知から起動した直後などは自分で求める。
// 起動した直後はアイコンがまだメニューバーに並んでおらず仮の位置が返るので、並ぶまで少し待つ（メインスレッドからは呼ばれない場面だけ）
fn place_below_tray(app: &AppHandle, window: &tauri::WebviewWindow) {
    const ATTEMPTS: u32 = 20;
    const INTERVAL: std::time::Duration = std::time::Duration::from_millis(50);
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    for _ in 0..ATTEMPTS {
        if let Some(rect) = tray.rect().ok().flatten() {
            // tray-icon は物理ピクセルで返すので、倍率をかけない（positioner と同じ扱い）
            let position = rect.position.to_physical::<f64>(1.0);
            if is_laid_out(position) {
                let width = window.outer_size().map(|size| size.width).unwrap_or(0);
                let _ = window.set_position(below_tray(position, rect.size.to_physical(1.0), width));
                return;
            }
        }
        std::thread::sleep(INTERVAL);
    }
}

fn is_laid_out(tray: PhysicalPosition<f64>) -> bool {
    tray.x > 0.0
}

fn below_tray(tray: PhysicalPosition<f64>, tray_size: PhysicalSize<f64>, window_width: u32) -> PhysicalPosition<i32> {
    PhysicalPosition::new(
        (tray.x + tray_size.width / 2.0) as i32 - (window_width / 2) as i32,
        tray.y as i32,
    )
}

// 監視のスレッドから window.is_visible() を呼ぶと、メインスレッドを待つ間に store のロックを持ったままになりうるので、開閉は自分で記録する
pub fn set_list_open(app: &AppHandle, open: bool) {
    app.state::<AppState>().list_open.store(open, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_opens_when_the_left_button_is_pressed() {
        // macOS のメニューバーのアイコンは、押した瞬間に開く。離すまで待つと、長めに押したときに遅れて見える
        assert!(opens_list(MouseButton::Left, MouseButtonState::Down));
        assert!(!opens_list(MouseButton::Left, MouseButtonState::Up));
        assert!(!opens_list(MouseButton::Right, MouseButtonState::Down));
    }

    #[test]
    fn list_is_centered_under_the_tray_icon() {
        let position = below_tray(PhysicalPosition::new(1000.0, 0.0), PhysicalSize::new(44.0, 48.0), 600);
        assert_eq!(position, PhysicalPosition::new(722, 0));
    }

    #[test]
    fn tray_rect_before_layout_is_not_used() {
        // 起動した直後は、メニューバーに並ぶ前の仮の位置（左端、画面の外）が返る
        assert!(!is_laid_out(PhysicalPosition::new(0.0, 2234.0)));
        assert!(is_laid_out(PhysicalPosition::new(2900.0, 0.0)));
    }
}
