use std::collections::HashMap;
use std::path::PathBuf;

use crate::agent::Agent;
use crate::ps::{is_agent_command, ProcRow};
use crate::state::Session;

const EDITOR_ENTRYPOINT: &str = "claude-vscode";
const MAX_DEPTH: usize = 32;

// エディターの会話のタブまでは選ばない。拡張の URI は「最後に使っていたウィンドウ」に届き、
// 開いたばかりのウィンドウとは限らないので、別のウィンドウに空の会話を作ってしまう（2026-10-11 に確認）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JumpTarget {
    App { app: PathBuf, folder: Option<String> },
    None,
}

// ps の args は実行ファイルのパスの空白を区切らずに持つので、空白で分けずに最初の ".app/Contents/" までを取る。
// 実行ファイルは必ず .app/Contents/ の下にあり、引数の中の ".app/" を取り違えにくい。
// 最初に現れる ".app" がいちばん外側（VS Code の拡張ホストは .app の中の .app で動く）
fn app_in_args(args: &str) -> Option<PathBuf> {
    if !args.starts_with('/') {
        return None;
    }
    let end = args.find(".app/Contents/")? + ".app".len();
    let app = &args[..end];
    // 空白の後ろから始まるパスや引数があれば、それは実行ファイルではなく引数の中の .app
    if app.contains(" /") || app.contains(" -") {
        return None;
    }
    Some(PathBuf::from(app))
}

pub fn outermost_app(rows: &[ProcRow], pid: u32) -> Option<PathBuf> {
    let by_pid: HashMap<u32, &ProcRow> = rows.iter().map(|r| (r.pid, r)).collect();
    let mut current = by_pid.get(&pid)?;
    for _ in 0..MAX_DEPTH {
        if let Some(app) = app_in_args(&current.args) {
            return Some(app);
        }
        if current.ppid == 0 || current.ppid == current.pid {
            return None;
        }
        current = by_pid.get(&current.ppid)?;
    }
    None
}

fn under_extension_host(rows: &[ProcRow], pid: u32) -> bool {
    let by_pid: HashMap<u32, &ProcRow> = rows.iter().map(|r| (r.pid, r)).collect();
    let mut current = by_pid.get(&pid).map(|r| r.ppid);
    for _ in 0..MAX_DEPTH {
        let Some(row) = current.and_then(|p| by_pid.get(&p)) else {
            return false;
        };
        // VS Code・Cursor・Windsurf の拡張は「… Helper (Plugin)」のプロセスで動く
        if row.args.contains(" Helper (Plugin)") {
            return true;
        }
        if row.ppid == 0 || row.ppid == row.pid {
            return false;
        }
        current = Some(row.ppid);
    }
    false
}

pub fn jump_target(session: &Session, rows: &[ProcRow]) -> JumpTarget {
    let terminal = session.terminal.as_ref();
    if terminal.is_some_and(|t| t.tmux_pane.is_some()) {
        return JumpTarget::None;
    }
    let Some(pid) = session.claude_pid else {
        return JumpTarget::None;
    };
    // 3 秒ごとの確認の間に pid が別のプロセスに使い回されると、関係のないアプリを前に出してしまう
    if !rows
        .iter()
        .find(|r| r.pid == pid)
        .is_some_and(|r| is_agent_command(&r.args))
    {
        return JumpTarget::None;
    }
    let Some(app) = outermost_app(rows, pid) else {
        return JumpTarget::None;
    };
    // エディターの中で動いているときは、そのフォルダーのウィンドウを前に出す。Codex は entrypoint がないので、親にエディターの拡張ホストがいるかで決める
    // （拡張に同梱の本体はターミナルからも動かせるので、本体の場所では決められない）
    let in_editor = match session.agent {
        Agent::ClaudeCode => terminal.and_then(|t| t.entrypoint.as_deref()) == Some(EDITOR_ENTRYPOINT),
        Agent::Codex => under_extension_host(rows, pid),
    };
    let folder = [&session.start_cwd, &session.cwd]
        .into_iter()
        .find(|dir| dir.starts_with('/'))
        .cloned();
    JumpTarget::App {
        app,
        folder: if in_editor { folder } else { None },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::TerminalInfo;
    use crate::state::Status;

    const VSCODE_HELPER: &str = "/Applications/Visual Studio Code.app/Contents/Frameworks/Code Helper (Plugin).app/Contents/MacOS/Code Helper (Plugin) --type=utility";
    const CLAUDE_IN_VSCODE: &str = "/Users/u/.vscode/extensions/anthropic.claude-code-2.1.296-darwin-arm64/resources/native-binary/claude --output-format stream-json";

    fn row(pid: u32, ppid: u32, args: &str) -> ProcRow {
        ProcRow {
            pid,
            ppid,
            tty: None,
            args: args.into(),
        }
    }

    fn vscode_rows() -> Vec<ProcRow> {
        vec![
            row(1, 0, "/sbin/launchd"),
            row(100, 1, "/Applications/Visual Studio Code.app/Contents/MacOS/Electron"),
            row(200, 100, VSCODE_HELPER),
            row(300, 200, CLAUDE_IN_VSCODE),
        ]
    }

    fn terminal_rows() -> Vec<ProcRow> {
        vec![
            row(1, 0, "/sbin/launchd"),
            row(
                10,
                1,
                "/System/Applications/Utilities/Terminal.app/Contents/MacOS/Terminal",
            ),
            row(11, 10, "login -pf u"),
            row(12, 11, "-zsh"),
            row(13, 12, "/Users/u/.local/bin/claude"),
        ]
    }

    fn session(id: &str, pid: u32, entrypoint: Option<&str>, tmux_pane: Option<&str>) -> Session {
        Session {
            id: id.into(),
            status: Status::Done,
            detail_kind: None,
            detail_text: None,
            status_since_ms: 0,
            first_seen_ms: 0,
            last_event_ms: 0,
            cwd: "/Users/u/Projects/api/sub".into(),
            start_cwd: "/Users/u/Projects/api".into(),
            config_dir: "/Users/u/.claude".into(),
            git_branch: None,
            claude_pid: Some(pid),
            terminal: Some(TerminalInfo {
                entrypoint: entrypoint.map(String::from),
                tmux_pane: tmux_pane.map(String::from),
                ..Default::default()
            }),
            transcript_path: None,
            running_subagents: Default::default(),
            agent: Default::default(),
            seen_ms: None,
            hidden: false,
        }
    }

    const ID: &str = "f94db67f-a0b2-4d0c-a9c9-b746bce595b2";

    #[test]
    fn takes_the_outermost_app_even_with_spaces_in_the_path() {
        assert_eq!(
            outermost_app(&vscode_rows(), 300),
            Some(PathBuf::from("/Applications/Visual Studio Code.app"))
        );
        assert_eq!(
            outermost_app(&terminal_rows(), 13),
            Some(PathBuf::from("/System/Applications/Utilities/Terminal.app"))
        );
        assert_eq!(outermost_app(&[row(5, 1, "tmux new"), row(6, 5, "claude")], 6), None);
        assert_eq!(outermost_app(&vscode_rows(), 999), None, "プロセスがもうない");
    }

    #[test]
    fn editors_open_the_folder_the_session_began_in() {
        let target = jump_target(&session(ID, 300, Some("claude-vscode"), None), &vscode_rows());
        assert_eq!(
            target,
            JumpTarget::App {
                app: PathBuf::from("/Applications/Visual Studio Code.app"),
                folder: Some("/Users/u/Projects/api".into())
            }
        );
        let mut unknown = session("pid:300", 300, Some("claude-vscode"), None);
        unknown.start_cwd = String::new();
        unknown.cwd = "-rf".into();
        assert_eq!(
            jump_target(&unknown, &vscode_rows()),
            JumpTarget::App {
                app: PathBuf::from("/Applications/Visual Studio Code.app"),
                folder: None
            },
            "絶対パスでないフォルダーは渡さない"
        );
    }

    #[test]
    fn terminals_only_come_to_the_front() {
        let target = jump_target(&session(ID, 13, Some("cli"), None), &terminal_rows());
        assert_eq!(
            target,
            JumpTarget::App {
                app: PathBuf::from("/System/Applications/Utilities/Terminal.app"),
                folder: None
            }
        );
    }

    #[test]
    fn tmux_and_missing_processes_have_no_target() {
        assert_eq!(
            jump_target(&session(ID, 13, Some("cli"), Some("%3")), &terminal_rows()),
            JumpTarget::None
        );
        assert_eq!(
            jump_target(&session(ID, 999, Some("cli"), None), &terminal_rows()),
            JumpTarget::None
        );
        let mut no_pid = session(ID, 13, Some("cli"), None);
        no_pid.claude_pid = None;
        assert_eq!(jump_target(&no_pid, &terminal_rows()), JumpTarget::None);
    }

    #[test]
    fn reused_pid_is_not_followed() {
        let rows = vec![
            row(1, 0, "/sbin/launchd"),
            row(100, 1, "/Applications/Visual Studio Code.app/Contents/MacOS/Electron"),
            row(300, 100, "/usr/bin/python3 server.py"),
        ];
        assert_eq!(
            jump_target(&session("s1", 300, Some("claude-vscode"), None), &rows),
            JumpTarget::None
        );
    }

    #[test]
    fn app_path_inside_an_argument_is_not_taken() {
        assert_eq!(app_in_args("/usr/local/bin/tool --open /Users/u/My.app/data"), None);
        assert_eq!(
            app_in_args("/usr/bin/login -fpl u /Applications/iTerm.app/Contents/MacOS/ShellLauncher --launch_shell"),
            None
        );
        assert_eq!(
            app_in_args("/Applications/iTerm.app/Contents/MacOS/iTerm2"),
            Some(PathBuf::from("/Applications/iTerm.app"))
        );
    }

    #[test]
    fn windsurf_opens_the_project_window() {
        let rows = vec![
            row(1, 0, "/sbin/launchd"),
            row(100, 1, "/Applications/Windsurf.app/Contents/MacOS/Electron"),
            row(200, 100, "/Applications/Windsurf.app/Contents/Frameworks/Windsurf Helper (Plugin).app/Contents/MacOS/Windsurf Helper (Plugin) --type=utility"),
            row(300, 200, "/Users/u/.windsurf/extensions/anthropic.claude-code-2.1.296-darwin-arm64/resources/native-binary/claude --output-format stream-json"),
        ];
        assert_eq!(
            jump_target(&session("s1", 300, Some("claude-vscode"), None), &rows),
            JumpTarget::App {
                app: PathBuf::from("/Applications/Windsurf.app"),
                folder: Some("/Users/u/Projects/api".into())
            }
        );
    }

    #[test]
    fn iterm2_brings_the_app_front() {
        let rows = vec![
            row(1, 0, "/sbin/launchd"),
            row(100, 1, "/Applications/iTerm.app/Contents/MacOS/iTerm2"),
            row(
                200,
                100,
                "/usr/bin/login -fpl u /Applications/iTerm.app/Contents/MacOS/ShellLauncher --launch_shell",
            ),
            row(300, 200, "-zsh"),
            row(400, 300, "/Users/u/.local/bin/claude"),
        ];
        assert_eq!(
            jump_target(&session("s1", 400, Some("cli"), None), &rows),
            JumpTarget::App {
                app: PathBuf::from("/Applications/iTerm.app"),
                folder: None
            }
        );
    }

    const CODEX_IN_VSCODE: &str = "/Users/u/.vscode/extensions/openai.chatgpt-26.1007.21434-darwin-arm64/bin/macos-aarch64/codex -c features.code_mode_host=true app-server";

    #[test]
    fn codex_in_vscode_opens_the_project_window() {
        let rows = vec![
            row(1, 0, "/sbin/launchd"),
            row(100, 1, "/Applications/Visual Studio Code.app/Contents/MacOS/Electron"),
            row(200, 100, VSCODE_HELPER),
            row(300, 200, CODEX_IN_VSCODE),
        ];
        let mut s = session("s1", 300, None, None);
        s.agent = crate::agent::Agent::Codex;
        assert_eq!(
            jump_target(&s, &rows),
            JumpTarget::App {
                app: PathBuf::from("/Applications/Visual Studio Code.app"),
                folder: Some("/Users/u/Projects/api".into())
            }
        );
    }

    #[test]
    fn codex_without_an_app_ancestor_does_not_jump() {
        // ターミナルの codex が裏のサーバー（launchd の子）で会話を動かしているとき
        let rows = vec![
            row(1, 0, "/sbin/launchd"),
            row(300, 1, "/opt/homebrew/bin/codex app-server"),
        ];
        let mut s = session("s1", 300, None, None);
        s.agent = crate::agent::Agent::Codex;
        assert_eq!(jump_target(&s, &rows), JumpTarget::None);
    }

    #[test]
    fn codex_from_the_extension_binary_in_a_terminal_brings_the_terminal_front() {
        // VS Code の拡張に同梱の本体をターミナルで動かしても、エディターではないのでフォルダーを開かない（開くと新しいウィンドウができる）
        let rows = vec![
            row(1, 0, "launchd"),
            row(10, 1, "/System/Applications/Utilities/Terminal.app/Contents/MacOS/Terminal"),
            row(11, 10, "login"),
            row(12, 11, "-zsh"),
            row(13, 12, "/Users/u/.vscode/extensions/openai.chatgpt-26.1007.21434-darwin-arm64/bin/macos-aarch64/codex --no-daemon"),
        ];
        let mut s = session("s1", 13, None, None);
        s.agent = crate::agent::Agent::Codex;
        assert_eq!(
            jump_target(&s, &rows),
            JumpTarget::App {
                app: PathBuf::from("/System/Applications/Utilities/Terminal.app"),
                folder: None
            }
        );
    }
}
