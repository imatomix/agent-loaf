use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::base::messages::{self, Lang, Msg};

pub const CLI_TIMEOUT: Duration = Duration::from_secs(60);
const REASON_MAX_CHARS: usize = 200;
const POLL_INTERVAL: Duration = Duration::from_millis(50);
// GUI アプリにはターミナルの PATH が引き継がれない。npm 版の claude は node を探すので、よくある場所も足す
const EXTRA_PATH: [&str; 6] = [
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "/usr/bin",
    "/bin",
    "/usr/sbin",
    "/sbin",
];

pub fn find_claude(configured: Option<&str>, home: &Path, is_executable: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    // 保存した場所は、claude を入れ直すと古くなる。使えなければ、よくある場所から探し直す
    if let Some(path) = configured.map(PathBuf::from).filter(|path| is_executable(path)) {
        return Some(path);
    }
    [
        home.join(".local/bin/claude"),
        PathBuf::from("/opt/homebrew/bin/claude"),
        PathBuf::from("/usr/local/bin/claude"),
        home.join(".npm-global/bin/claude"),
        home.join(".claude/local/claude"),
    ]
    .into_iter()
    .find(|path| is_executable(path))
}

const CODEX_EXTENSION_ROOTS: [&str; 3] = [".vscode/extensions", ".cursor/extensions", ".windsurf/extensions"];

pub fn find_codex(configured: Option<&str>, home: &Path, is_executable: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    if let Some(path) = configured.map(PathBuf::from).filter(|path| is_executable(path)) {
        return Some(path);
    }
    let in_path = [
        home.join(".local/bin/codex"),
        PathBuf::from("/opt/homebrew/bin/codex"),
        PathBuf::from("/usr/local/bin/codex"),
        home.join(".npm-global/bin/codex"),
    ];
    if let Some(path) = in_path.into_iter().find(|path| is_executable(path)) {
        return Some(path);
    }
    // VS Code などの Codex の拡張は、本体を拡張のフォルダーに同梱している。古い版のフォルダーが残ることがあるので、新しい版から試す
    let mut bundled: Vec<(String, PathBuf)> = CODEX_EXTENSION_ROOTS
        .iter()
        .filter_map(|root| std::fs::read_dir(home.join(root)).ok())
        .flatten()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let version = name.strip_prefix("openai.chatgpt-")?.to_string();
            Some((version, entry.path().join("bin/macos-aarch64/codex")))
        })
        .collect();
    bundled.sort_by(|a, b| compare_versions(&b.0, &a.0));
    bundled
        .into_iter()
        .map(|(_, path)| path)
        .find(|path| is_executable(path))
}

fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parts = |v: &str| {
        v.split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .map(|s| s.parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    parts(a).cmp(&parts(b))
}

pub fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

pub fn failure_reason(stderr: &str, stdout: &str) -> Option<String> {
    let last_line = |text: &str| {
        text.lines()
            .rev()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .map(str::to_string)
    };
    let reason = last_line(stderr).or_else(|| last_line(stdout))?;
    Some(reason.chars().take(REASON_MAX_CHARS).collect())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    Failed(String),
    Timeout,
    Spawn(String),
    Wait(String),
    Unreadable,
}

impl CliError {
    pub fn message(&self, lang: Lang) -> String {
        match self {
            CliError::Failed(reason) if !reason.is_empty() => reason.clone(),
            CliError::Failed(_) => messages::text(lang, Msg::ErrorUnknownReason),
            CliError::Timeout => messages::text(lang, Msg::ErrorTimeout),
            CliError::Spawn(error) => messages::format(lang, Msg::ErrorCliSpawn, &[("error", error)]),
            CliError::Wait(error) => messages::format(lang, Msg::ErrorCliWait, &[("error", error)]),
            CliError::Unreadable => messages::text(lang, Msg::ErrorUnreadableStatus),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClaudeCli {
    program: PathBuf,
    config_env: &'static str,
}

impl ClaudeCli {
    pub fn new(program: PathBuf) -> Self {
        ClaudeCli {
            program,
            config_env: "CLAUDE_CONFIG_DIR",
        }
    }

    // 同じ実行の仕組みで codex も動かす。設定ディレクトリを渡す環境変数の名前だけが違う
    pub fn with_config_env(mut self, name: &'static str) -> Self {
        self.config_env = name;
        self
    }

    pub fn run(&self, config_dir: &Path, args: &[String], timeout: Duration) -> Result<String, CliError> {
        let path = self.path_env();
        run_command(
            &self.program,
            args,
            &[(self.config_env, config_dir.as_os_str()), ("PATH", OsStr::new(&path))],
            timeout,
        )
    }

    fn path_env(&self) -> String {
        let mut parts: Vec<String> = self
            .program
            .parent()
            .map(|dir| dir.to_string_lossy().into_owned())
            .into_iter()
            .collect();
        parts.extend(EXTRA_PATH.iter().map(|dir| dir.to_string()));
        parts.join(":")
    }
}

pub fn run_command(
    program: &Path,
    args: &[String],
    envs: &[(&str, &OsStr)],
    timeout: Duration,
) -> Result<String, CliError> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (name, value) in envs {
        command.env(name, value);
    }
    let mut child = command.spawn().map_err(|error| CliError::Spawn(error.to_string()))?;
    // 出力を読みながら待たないと、出力が多いときにパイプが詰まって終わらなくなる
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(CliError::Timeout);
            }
            Ok(None) => thread::sleep(POLL_INTERVAL),
            Err(error) => return Err(CliError::Wait(error.to_string())),
        }
    };
    let (stdout, stderr) = (stdout.join().unwrap_or_default(), stderr.join().unwrap_or_default());
    if status.success() {
        Ok(stdout)
    } else {
        Err(CliError::Failed(failure_reason(&stderr, &stdout).unwrap_or_default()))
    }
}

fn drain(pipe: Option<impl Read + Send + 'static>) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut text = String::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_string(&mut text);
        }
        text
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }

    #[test]
    fn configured_path_wins_then_known_locations_in_order() {
        let home = Path::new("/Users/u");
        let exists =
            |p: &Path| p == Path::new("/opt/homebrew/bin/claude") || p == Path::new("/Users/u/.local/bin/claude");
        assert_eq!(
            find_claude(None, home, exists),
            Some(PathBuf::from("/Users/u/.local/bin/claude"))
        );
        assert_eq!(
            find_claude(Some("/opt/homebrew/bin/claude"), home, exists),
            Some(PathBuf::from("/opt/homebrew/bin/claude"))
        );
        assert_eq!(find_claude(Some("/nowhere/claude"), home, |_| false), None);
    }

    #[test]
    fn stale_configured_path_falls_back_to_auto_detection() {
        let home = Path::new("/Users/u");
        let found = find_claude(Some("/old/claude"), home, |p| {
            p == Path::new("/opt/homebrew/bin/claude")
        });
        assert_eq!(found, Some(PathBuf::from("/opt/homebrew/bin/claude")));
        let configured = find_claude(Some("/my/claude"), home, |p| {
            p == Path::new("/my/claude") || p == Path::new("/opt/homebrew/bin/claude")
        });
        assert_eq!(
            configured,
            Some(PathBuf::from("/my/claude")),
            "使える場所が保存されていればそれを使う"
        );
    }

    #[test]
    fn passes_config_dir_and_returns_stdout() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(dir.path(), "claude", r#"echo "$CLAUDE_CONFIG_DIR $1""#);
        let out = ClaudeCli::new(program)
            .run(Path::new("/cfg/a"), &["plugin".into()], CLI_TIMEOUT)
            .unwrap();
        assert_eq!(out.trim(), "/cfg/a plugin");
    }

    #[test]
    fn program_directory_is_on_path() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(dir.path(), "claude", r#"echo "$PATH""#);
        let out = ClaudeCli::new(program)
            .run(Path::new("/cfg"), &[], CLI_TIMEOUT)
            .unwrap();
        assert!(out.split(':').next().unwrap().trim() == dir.path().to_str().unwrap());
    }

    #[test]
    fn failure_reports_last_stderr_line() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(
            dir.path(),
            "claude",
            "echo progress; echo 'first' >&2; echo 'blocked by policy' >&2; exit 1",
        );
        let error = ClaudeCli::new(program)
            .run(Path::new("/cfg"), &[], CLI_TIMEOUT)
            .unwrap_err();
        assert_eq!(error, CliError::Failed("blocked by policy".into()));
    }

    #[test]
    fn times_out_and_kills_the_process() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(dir.path(), "claude", "sleep 5");
        let started = std::time::Instant::now();
        let error = ClaudeCli::new(program)
            .run(Path::new("/cfg"), &[], Duration::from_millis(200))
            .unwrap_err();
        assert_eq!(error, CliError::Timeout);
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn reason_is_trimmed_to_200_chars_and_falls_back_to_stdout() {
        let long = "あ".repeat(300);
        assert_eq!(failure_reason(&format!("{long}\n\n"), "").unwrap().chars().count(), 200);
        assert_eq!(failure_reason("", "only stdout\n").as_deref(), Some("only stdout"));
        assert_eq!(failure_reason("", ""), None);
    }

    #[test]
    fn error_messages_follow_the_language() {
        use crate::base::messages::Lang;
        assert_eq!(CliError::Timeout.message(Lang::En), "Timed out (no response)");
        assert_eq!(
            CliError::Failed("blocked".into()).message(Lang::Ja),
            "blocked",
            "claude 自身の文は訳さない"
        );
        assert_eq!(CliError::Failed(String::new()).message(Lang::En), "Unknown reason");
        assert_eq!(CliError::Unreadable.message(Lang::Ja), "導入状態を読めませんでした");
    }

    #[test]
    fn finds_codex_in_path_dirs_then_the_newest_editor_extension() {
        let home = tempfile::tempdir().unwrap();
        let ext = |editor: &str, version: &str| {
            home.path().join(format!(
                "{editor}/extensions/openai.chatgpt-{version}-darwin-arm64/bin/macos-aarch64/codex"
            ))
        };
        for path in [
            ext(".vscode", "26.1007.21434"),
            ext(".vscode", "26.0901.1"),
            ext(".cursor", "25.1.1"),
        ] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, "").unwrap();
        }
        let exists = |p: &Path| p.is_file();
        assert_eq!(
            find_codex(None, home.path(), exists),
            Some(ext(".vscode", "26.1007.21434")),
            "いちばん新しい版"
        );
        let local = home.path().join(".local/bin/codex");
        std::fs::create_dir_all(local.parent().unwrap()).unwrap();
        std::fs::write(&local, "").unwrap();
        assert_eq!(
            find_codex(None, home.path(), exists),
            Some(local.clone()),
            "PATH のよくある場所を先に"
        );
        assert_eq!(
            find_codex(Some("/old/codex"), home.path(), exists),
            Some(local),
            "保存した場所が古ければ探し直す"
        );
    }
}
