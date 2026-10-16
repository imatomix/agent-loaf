use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn monitor_home() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("AGENT_LOAF_HOME") {
        return Some(PathBuf::from(dir));
    }
    Some(dirs::home_dir()?.join(".agent-loaf"))
}

pub fn prepare_private_dir(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

// フォルダーは 700 だが、中のファイルも本人だけにしておく（README の「本人だけが読み書きできる」に合わせる）。
// mode は新しく作るときだけ効く。書くファイルは一時ファイルに書いてから名前を変えるので、毎回新しく作られる
pub fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(bytes)
}

pub fn crash_log_path(home: &Path) -> PathBuf {
    home.join("crash.log")
}

pub fn events_dir(home: &Path) -> PathBuf {
    home.join("events")
}

pub fn raw_dir(home: &Path) -> PathBuf {
    home.join("raw")
}

pub fn bin_path(home: &Path) -> PathBuf {
    home.join("bin").join("agent-loaf-hook")
}

pub fn marketplace_dir(home: &Path) -> PathBuf {
    home.join("marketplace")
}

pub fn state_path(home: &Path) -> PathBuf {
    home.join("state.json")
}

pub fn settings_path(home: &Path) -> PathBuf {
    home.join("settings.json")
}

pub fn default_config_dir() -> Option<String> {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        return Some(dir.to_string_lossy().into_owned());
    }
    Some(dirs::home_dir()?.join(".claude").to_string_lossy().into_owned())
}

pub fn default_codex_dir() -> Option<String> {
    if let Some(dir) = std::env::var_os("CODEX_HOME") {
        return Some(dir.to_string_lossy().into_owned());
    }
    Some(dirs::home_dir()?.join(".codex").to_string_lossy().into_owned())
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn private_files_are_readable_only_by_the_owner() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f.json");
        write_private(&path, b"{}").unwrap();
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        write_private(&path, b"[]").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"[]");
    }

    #[test]
    fn codex_dir_defaults_to_dot_codex() {
        let dir = default_codex_dir().unwrap();
        match std::env::var_os("CODEX_HOME") {
            Some(home) => assert_eq!(dir, home.to_string_lossy()),
            None => assert!(dir.ends_with("/.codex"), "{dir}"),
        }
    }
}
