mod base;
mod commands;
mod process;
mod session;
mod setup;
mod tray;
mod update;

use agent_loaf_core::base::crash_log;
use agent_loaf_core::base::paths;
use tauri_plugin_autostart::MacosLauncher;

pub fn run() {
    let home = paths::monitor_home().expect("home directory not found");
    let crash_log = paths::crash_log_path(&home);
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crash_log::append(&crash_log, paths::now_ms(), &info.to_string());
        default_hook(info);
    }));
    if let Err(error) =
        paths::prepare_private_dir(&home).and_then(|_| paths::prepare_private_dir(&paths::events_dir(&home)))
    {
        eprintln!("failed to create {}: {error}", home.display());
    }
    session::monitor::install_bundled_hook(&home);

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None));
    // 更新のプラグインは、設定に公開鍵がないと起動できない。開発のビルドでは登録しない
    if update::updater::ENABLED {
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }
    builder
        .manage(update::updater::UpdaterState::default())
        .manage(session::monitor::load_state(home))
        .manage(setup::installer::InstallerState::default())
        .invoke_handler(tauri::generate_handler![
            commands::session::get_sessions,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::session::move_session,
            commands::setup::get_setup,
            commands::setup::add_config_dir,
            commands::setup::retry_config_dir,
            commands::setup::remove_config_dir,
            commands::setup::set_claude_path,
            commands::setup::set_codex_path,
            commands::setup::open_codex_trust,
            commands::setup::refresh_setup,
            commands::settings::open_notification_settings,
            commands::app::get_messages,
            commands::session::jump_session,
            commands::session::take_pending_details,
            commands::settings::refresh_notification_access,
            commands::update::get_update,
            commands::update::is_release_build,
            commands::update::install_update,
            commands::app::quit_app,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            // 通知を押されてアプリが起動したとき、その知らせはここで準備した直後に届くことがある。
            // 一覧をアイコンの下に開くにはアイコンが要るので、先に作っておく
            tray::setup_tray(app)?;
            tray::setup_windows(app);
            let handle = app.handle().clone();
            let on_open_handle = app.handle().clone();
            session::notifier::init(
                move || setup::installer::publish(&handle),
                move |session_id| {
                    let app = on_open_handle.clone();
                    // 通知の処理は macOS のスレッドで呼ばれるので、ps や open は別のスレッドで動かす
                    std::thread::spawn(move || {
                        if !process::jumper::jump(&app, &session_id) {
                            tray::show_details(&app, &session_id);
                        }
                    });
                },
            );
            session::monitor::publish(app.handle());
            session::monitor::spawn(app.handle().clone());
            setup::installer::spawn(app.handle().clone());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build agent-loaf")
        .run(|_app, event| {
            // 一覧を閉じるとウィンドウがなくなるが、メニューバーのアプリとして動き続ける
            if let tauri::RunEvent::ExitRequested { api, code: None, .. } = event {
                api.prevent_exit();
            }
        });
}
