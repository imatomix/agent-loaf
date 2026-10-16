use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Mutex;
use std::thread;

use agent_loaf_core::agent::{agent_of_config_dir, Agent};
use agent_loaf_core::attention::needs_attention;
use agent_loaf_core::claude_cli::{find_claude, find_codex, is_executable, ClaudeCli, CliError, CLI_TIMEOUT};
use agent_loaf_core::codex_plugin::{hooks_trusted, parse_codex_inspection};
use agent_loaf_core::config_dirs::{display_path, initial_config_dirs, is_codex_dir, offer_codex_dir};
use agent_loaf_core::install_plan::{
    install_steps, remove_steps, update_steps, CliStep, INSPECT_MARKETPLACES, INSPECT_PLUGINS,
};
use agent_loaf_core::marketplace::{bundled_plugin_version, sync_marketplace};
use agent_loaf_core::messages::{self, Lang, Msg};
use agent_loaf_core::paths;
use agent_loaf_core::persist::save_json;
use agent_loaf_core::plugin_status::{parse_inspection, DirState, Inspection};
use agent_loaf_core::settings::Settings;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

use crate::monitor::AppState;
use crate::notifier;
use crate::sync::LockExt;

pub enum Job {
    Startup,
    Add(String),
    Retry(String),
    Remove(String),
    Refresh,
}

// 設定（AppState.settings）のロックを持ったまま、ここのロックを取らないこと。view は逆の順で取らないよう、先に設定を読み終える
#[derive(Default)]
pub struct InstallerState {
    sender: Mutex<Option<Sender<Job>>>,
    inspections: Mutex<HashMap<String, Result<Inspection, CliError>>>,
    busy: Mutex<HashSet<String>>,
    // 設定画面を開くたびに積まれるが、まだ始まっていない再読み込みが 1 つあれば足りる
    refresh_queued: std::sync::atomic::AtomicBool,
    claude: Mutex<Option<PathBuf>>,
    codex: Mutex<Option<PathBuf>>,
    // Codex の設定ディレクトリごとの、hook が許可されているかの判断（config.toml を読んだ結果）
    trust: Mutex<HashMap<String, bool>>,
    marketplace_error: Mutex<Option<String>>,
    // 起動直後は画面の準備より先に起動の処理が終わり、イベントでは取りこぼすので、画面が読む状態として持つ
    open_settings_on_start: std::sync::atomic::AtomicBool,
    // 起動の処理（claude を探し、設定ディレクトリを決めて調べる）が済むまでは、見つからない・入っていないと決めつけない
    startup_done: std::sync::atomic::AtomicBool,
}

enum InstallError {
    Cli(CliError),
    NotFound(Agent),
    MarketplaceUnavailable(String),
}

impl From<CliError> for InstallError {
    fn from(error: CliError) -> Self {
        InstallError::Cli(error)
    }
}

impl InstallError {
    fn message(&self, lang: Lang) -> String {
        match self {
            InstallError::Cli(error) => error.message(lang),
            InstallError::NotFound(Agent::ClaudeCode) => messages::text(lang, Msg::ErrorClaudeNotFound),
            InstallError::NotFound(Agent::Codex) => messages::text(lang, Msg::ErrorCodexNotFound),
            InstallError::MarketplaceUnavailable(error) => {
                messages::format(lang, Msg::ErrorMarketplaceUnavailable, &[("error", error)])
            }
        }
    }
}

fn failure_message(app: &AppHandle, msg: Msg, path: &str, error: &InstallError) -> String {
    let lang = app.state::<AppState>().lang;
    messages::format(lang, msg, &[("dir", &display(path)), ("reason", &error.message(lang))])
}

#[derive(Clone, Serialize)]
pub struct ConfigDirView {
    pub path: String,
    pub display: String,
    pub state: DirState,
    pub busy: bool,
    pub agent: Agent,
}

#[derive(Clone, Serialize)]
pub struct SetupView {
    pub dirs: Vec<ConfigDirView>,
    pub claude_found: bool,
    pub codex_found: bool,
    pub notifications_allowed: bool,
    pub needs_attention: bool,
    pub open_settings_on_start: bool,
}

pub fn spawn(app: AppHandle) {
    let (sender, receiver) = mpsc::channel();
    *app.state::<InstallerState>().sender.locked() = Some(sender.clone());
    thread::spawn(move || run(app, receiver));
    let _ = sender.send(Job::Startup);
}

pub fn send(app: &AppHandle, job: Job) {
    let installer = app.state::<InstallerState>();
    if matches!(job, Job::Refresh) && installer.refresh_queued.swap(true, Ordering::Relaxed) {
        return;
    }
    let sender = installer.sender.locked();
    if let Some(sender) = sender.as_ref() {
        let _ = sender.send(job);
    }
}

// claude の呼び出しを重ねないよう、すべての仕事をこの 1 本のスレッドで順に行う
fn run(app: AppHandle, receiver: Receiver<Job>) {
    for job in receiver {
        match job {
            Job::Startup => startup(&app),
            Job::Add(path) => add(&app, path),
            Job::Retry(path) => user_action(&app, &path, install_steps),
            Job::Remove(path) => remove(&app, &path),
            Job::Refresh => {
                app.state::<InstallerState>()
                    .refresh_queued
                    .store(false, Ordering::Relaxed);
                refresh_all(&app);
            }
        }
        publish(&app);
        // 監視の対象が変わると、一覧・メニューバーのアイコンに出すセッションも変わる
        crate::monitor::publish(&app);
    }
}

fn startup(app: &AppHandle) {
    let state = app.state::<AppState>();
    let source = app.path().resource_dir().map(|dir| dir.join("marketplace"));
    let result = source.map_err(|error| error.to_string()).and_then(|source| {
        sync_marketplace(&source, &paths::marketplace_dir(&state.home)).map_err(|error| error.to_string())
    });
    *app.state::<InstallerState>().marketplace_error.locked() = result.err();
    resolve_tools(app);

    let auto_dir = {
        let home = dirs::home_dir().unwrap_or_default();
        let decided = update_settings(app, |settings| {
            let decided = initial_config_dirs(settings.config_dirs.as_deref(), &home.join(".claude"));
            if let Some(dirs) = decided.clone() {
                settings.config_dirs = Some(dirs);
            }
            decided
        });
        decided.and_then(|dirs| dirs.into_iter().next())
    };
    if let Some(dir) = auto_dir {
        let _ = apply(app, &dir, install_steps);
    }
    // ~/.codex は、すでに設定ディレクトリを決めている人にも 1 回だけ足す。外した後は足さない。ないときは覚えず、後で Codex を入れた人にも足す
    let offered_codex = paths::default_codex_dir().map(PathBuf::from).and_then(|codex| {
        update_settings(app, |settings| {
            let next = offer_codex_dir(settings.config_dirs.as_deref(), settings.codex_dir_offered, &codex)?;
            settings.config_dirs = Some(next);
            settings.codex_dir_offered = true;
            Some(codex.to_string_lossy().into_owned())
        })
    });
    if let Some(dir) = offered_codex {
        let _ = apply(app, &dir, install_steps);
    }
    // apply が最初に状態を読むので、前もって全部を読み直さない
    match bundled_plugin_version(&paths::marketplace_dir(&state.home)) {
        Some(version) => {
            for dir in config_dirs(app) {
                let _ = apply(app, &dir, |inspection| update_steps(inspection, &version));
            }
        }
        None => refresh_all(app),
    }
    app.state::<InstallerState>()
        .startup_done
        .store(true, Ordering::Relaxed);
    if config_dirs(app).is_empty() {
        app.state::<InstallerState>()
            .open_settings_on_start
            .store(true, Ordering::Relaxed);
        crate::tray::show_list(app);
    }
}

fn add(app: &AppHandle, path: String) {
    let added = update_settings(app, |settings| {
        let mut dirs = settings.config_dirs.clone().unwrap_or_default();
        if dirs.contains(&path) {
            return false;
        }
        dirs.push(path.clone());
        settings.config_dirs = Some(dirs);
        remember_codex_choice(settings, &path);
        true
    });
    if added {
        user_action(app, &path, install_steps);
    }
}

fn remove(app: &AppHandle, path: &str) {
    if Path::new(path).is_dir() {
        match apply(app, path, remove_steps) {
            Ok(()) => {}
            // claude や codex がないとプラグインは外せないが、一覧から外せないと消す手段がなくなる
            Err(InstallError::NotFound(agent)) => {
                let lang = app.state::<AppState>().lang;
                let msg = match agent {
                    Agent::ClaudeCode => Msg::InfoRemovedWithoutClaude,
                    Agent::Codex => Msg::InfoRemovedWithoutCodex,
                };
                alert(app, &messages::format(lang, msg, &[("dir", &display(path))]));
            }
            Err(reason) => {
                alert(app, &failure_message(app, Msg::ErrorRemoveFailed, path, &reason));
                return;
            }
        }
    }
    update_settings(app, |settings| {
        let dirs = settings
            .config_dirs
            .clone()
            .unwrap_or_default()
            .into_iter()
            .filter(|dir| dir != path)
            .collect();
        settings.config_dirs = Some(dirs);
        remember_codex_choice(settings, path);
    });
    app.state::<InstallerState>().inspections.locked().remove(path);
    app.state::<InstallerState>().trust.locked().remove(path);
}

// ~/.codex を手で足したり外したりした人には、起動したときに自動で足し直さない
fn remember_codex_choice(settings: &mut Settings, path: &str) {
    if paths::default_codex_dir().is_some_and(|codex| is_codex_dir(path, Path::new(&codex))) {
        settings.codex_dir_offered = true;
    }
}

fn user_action(app: &AppHandle, path: &str, steps: impl Fn(&Inspection) -> Vec<CliStep>) {
    if let Err(reason) = apply(app, path, steps) {
        alert(app, &failure_message(app, Msg::ErrorInstallFailed, path, &reason));
    }
}

// 状態を読み直してから手順を決め、1 つずつ実行する。終わったらもう一度読み直して画面に反映する
fn apply(app: &AppHandle, path: &str, steps: impl Fn(&Inspection) -> Vec<CliStep>) -> Result<(), InstallError> {
    // ないフォルダーに claude を動かしても失敗するだけ。画面には「見つからない」と出る
    if !Path::new(path).is_dir() {
        return Ok(());
    }
    let installer = app.state::<InstallerState>();
    let agent = agent_of_config_dir(Path::new(path));
    let cli = cli(app, agent).ok_or(InstallError::NotFound(agent))?;
    // 状態を読む間も含めて押せなくする。読み終わるまで押せると、同じ操作が重ねて積まれる
    installer.busy.locked().insert(path.to_string());
    publish(app);
    let result = run_plan(app, &cli, agent, path, steps);
    installer.busy.locked().remove(path);
    // 何も実行しなかったときは、計画を決めるときに読んだ状態がそのまま今の状態
    if !matches!(result, Ok(0)) {
        let _ = inspect(app, &cli, agent, path);
    }
    result.map(|_| ())
}

fn run_plan(
    app: &AppHandle,
    cli: &ClaudeCli,
    agent: Agent,
    path: &str,
    steps: impl Fn(&Inspection) -> Vec<CliStep>,
) -> Result<usize, InstallError> {
    let plan = steps(&inspect(app, cli, agent, path)?);
    if plan.iter().any(CliStep::needs_bundled_copy) {
        if let Some(error) = app.state::<InstallerState>().marketplace_error.locked().clone() {
            return Err(InstallError::MarketplaceUnavailable(error));
        }
    }
    let bundled = paths::marketplace_dir(&app.state::<AppState>().home);
    plan.iter().try_for_each(|step| {
        cli.run(Path::new(path), &step.args_for(agent, &bundled), CLI_TIMEOUT)
            .map(|_| ())
    })?;
    Ok(plan.len())
}

fn inspect(app: &AppHandle, cli: &ClaudeCli, agent: Agent, path: &str) -> Result<Inspection, CliError> {
    let bundled = paths::marketplace_dir(&app.state::<AppState>().home);
    let to_args = |words: &[&str]| words.iter().map(|word| word.to_string()).collect::<Vec<_>>();
    let result = cli
        .run(Path::new(path), &to_args(&INSPECT_PLUGINS), CLI_TIMEOUT)
        .and_then(|plugins| {
            let marketplaces = cli.run(Path::new(path), &to_args(&INSPECT_MARKETPLACES), CLI_TIMEOUT)?;
            match agent {
                Agent::ClaudeCode => parse_inspection(&plugins, &marketplaces, &bundled),
                Agent::Codex => parse_codex_inspection(&plugins, &marketplaces, &bundled),
            }
        });
    if agent == Agent::Codex {
        check_trust(app, path);
    }
    app.state::<InstallerState>()
        .inspections
        .locked()
        .insert(path.to_string(), result.clone());
    result
}

// config.toml は読むだけ。書き換えると Codex の許可の仕組みを回り道することになる
fn check_trust(app: &AppHandle, path: &str) {
    let config = std::fs::read_to_string(Path::new(path).join("config.toml")).unwrap_or_default();
    let hooks_json =
        std::fs::read_to_string(paths::marketplace_dir(&app.state::<AppState>().home).join("plugin/hooks/hooks.json"))
            .unwrap_or_default();
    let registered: Vec<String> = serde_json::from_str::<serde_json::Value>(&hooks_json)
        .ok()
        .and_then(|v| v.get("hooks")?.as_object().map(|m| m.keys().cloned().collect()))
        .unwrap_or_default();
    let registered: Vec<&str> = registered.iter().map(String::as_str).collect();
    app.state::<InstallerState>()
        .trust
        .locked()
        .insert(path.to_string(), hooks_trusted(&config, &registered));
}

fn refresh_all(app: &AppHandle) {
    resolve_tools(app);
    for dir in config_dirs(app) {
        if !Path::new(&dir).is_dir() {
            continue;
        }
        let agent = agent_of_config_dir(Path::new(&dir));
        if let Some(cli) = cli(app, agent) {
            let _ = inspect(app, &cli, agent, &dir);
        }
    }
}

fn resolve_tools(app: &AppHandle) {
    let state = app.state::<AppState>();
    let (claude_path, codex_path) = {
        let settings = state.settings.locked();
        (settings.claude_path.clone(), settings.codex_path.clone())
    };
    let home = dirs::home_dir().unwrap_or_default();
    let installer = app.state::<InstallerState>();
    *installer.claude.locked() = find_claude(claude_path.as_deref(), &home, is_executable);
    *installer.codex.locked() = find_codex(codex_path.as_deref(), &home, is_executable);
}

fn cli(app: &AppHandle, agent: Agent) -> Option<ClaudeCli> {
    let installer = app.state::<InstallerState>();
    match agent {
        Agent::ClaudeCode => installer.claude.locked().clone().map(ClaudeCli::new),
        Agent::Codex => installer
            .codex
            .locked()
            .clone()
            .map(|path| ClaudeCli::new(path).with_config_env("CODEX_HOME")),
    }
}

pub fn codex_path(app: &AppHandle) -> Option<PathBuf> {
    app.state::<InstallerState>().codex.locked().clone()
}

fn config_dirs(app: &AppHandle) -> Vec<String> {
    app.state::<AppState>()
        .settings
        .locked()
        .config_dirs
        .clone()
        .unwrap_or_default()
}

fn update_settings<T>(app: &AppHandle, change: impl FnOnce(&mut Settings) -> T) -> T {
    let state = app.state::<AppState>();
    let mut settings = state.settings.locked();
    let result = change(&mut settings);
    if let Err(error) = save_json(&paths::settings_path(&state.home), &*settings) {
        eprintln!("failed to save settings: {error}");
    }
    result
}

fn display(path: &str) -> String {
    display_path(path, &dirs::home_dir().unwrap_or_default())
}

pub fn view(app: &AppHandle) -> SetupView {
    let paths = config_dirs(app);
    let installer = app.state::<InstallerState>();
    let started = installer.startup_done.load(Ordering::Relaxed);
    let claude_missing = installer.claude.locked().is_none();
    let codex_missing = installer.codex.locked().is_none();
    let inspections = installer.inspections.locked();
    let trust = installer.trust.locked();
    let busy = installer.busy.locked();
    let dirs: Vec<ConfigDirView> = paths
        .into_iter()
        .map(|path| {
            let agent = agent_of_config_dir(Path::new(&path));
            let tool_missing = match agent {
                Agent::ClaudeCode => claude_missing,
                Agent::Codex => codex_missing,
            };
            let state = if !Path::new(&path).is_dir() {
                DirState::Missing
            } else {
                match inspections.get(&path) {
                    Some(Ok(inspection))
                        if agent == Agent::Codex
                            && inspection.state() == DirState::Installed
                            && trust.get(&path) == Some(&false) =>
                    {
                        DirState::NeedsTrust
                    }
                    Some(Ok(inspection)) => inspection.state(),
                    Some(Err(_)) => DirState::NotInstalled,
                    // claude や codex がなければ調べられないので、いつまでも「確かめています」にしない
                    None if started && tool_missing => DirState::NotInstalled,
                    None => DirState::Checking,
                }
            };
            ConfigDirView {
                display: display(&path),
                busy: busy.contains(&path),
                path,
                state,
                agent,
            }
        })
        .collect();
    // 使っていないエージェントのコマンドがなくても、案内や赤い丸は出さない
    let uses = |agent: Agent| dirs.iter().any(|dir| dir.agent == agent);
    let claude_found = !(uses(Agent::ClaudeCode) && claude_missing);
    let codex_found = !(uses(Agent::Codex) && codex_missing);
    let tools_found = started.then_some(claude_found && codex_found);
    let notifications_allowed = notifier::notifications_allowed();
    let states: Vec<DirState> = dirs.iter().map(|dir| dir.state).collect();
    SetupView {
        needs_attention: needs_attention(started.then_some(&states[..]), tools_found, notifications_allowed),
        open_settings_on_start: installer.open_settings_on_start.swap(false, Ordering::Relaxed),
        dirs,
        claude_found: !started || claude_found,
        codex_found: !started || codex_found,
        notifications_allowed: notifications_allowed != Some(false),
    }
}

pub fn publish(app: &AppHandle) {
    let _ = app.emit("setup-updated", view(app));
}

pub fn alert(app: &AppHandle, message: &str) {
    app.state::<AppState>().suppress_hide.store(true, Ordering::Relaxed);
    let _ = app.emit("keep-page", ());
    let handle = app.clone();
    app.dialog()
        .message(message)
        .title("Agent Loaf")
        .kind(MessageDialogKind::Error)
        .show(move |_| handle.state::<AppState>().suppress_hide.store(false, Ordering::Relaxed));
}
