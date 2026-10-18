use std::collections::HashSet;
use std::path::Path;

use crate::store::UNKNOWN_ID_PREFIX;

// 監視の対象（「Claude Code の設定」の一覧）で動いているセッションだけを表示・通知するための照合
#[derive(Debug, Clone, Default)]
pub struct Monitored {
    dirs: HashSet<String>,
}

impl Monitored {
    pub fn new(dirs: &[String]) -> Self {
        Monitored {
            dirs: dirs.iter().map(|dir| normalized(dir)).collect(),
        }
    }

    pub fn includes(&self, session_id: &str, config_dir: &str) -> bool {
        session_id.starts_with(UNKNOWN_ID_PREFIX) || self.dirs.contains(&normalized(config_dir))
    }
}

fn normalized(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    std::fs::canonicalize(trimmed)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| trimmed.to_string())
}

pub fn initial_config_dirs(current: Option<&[String]>, default_dir: &Path) -> Option<Vec<String>> {
    if current.is_some() || !default_dir.is_dir() {
        return None;
    }
    Some(vec![default_dir.to_string_lossy().into_owned()])
}

pub fn offer_codex_dir(current: Option<&[String]>, offered: bool, codex_dir: &Path) -> Option<Vec<String>> {
    let codex = codex_dir.to_string_lossy().into_owned();
    if offered || !codex_dir.is_dir() || current.is_some_and(|dirs| lists_dir(dirs, &codex)) {
        return None;
    }
    let mut dirs = current.map(<[String]>::to_vec).unwrap_or_default();
    dirs.push(codex);
    Some(dirs)
}

// 末尾の '/' やシンボリックリンクで書き方が違っても、同じフォルダーなら同じとみなす
pub fn lists_dir(dirs: &[String], path: &str) -> bool {
    let wanted = normalized(path);
    dirs.iter().any(|dir| normalized(dir) == wanted)
}

pub fn is_codex_dir(path: &str, codex_dir: &Path) -> bool {
    normalized(path) == normalized(&codex_dir.to_string_lossy())
}

pub fn display_path(path: &str, home: &Path) -> String {
    let home = home.to_string_lossy();
    match path.strip_prefix(home.as_ref()) {
        Some("") => "~".into(),
        Some(rest) if rest.starts_with('/') => format!("~{rest}"),
        _ => path.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decides_only_while_unset_and_default_exists() {
        let home = tempfile::tempdir().unwrap();
        let claude = home.path().join(".claude");
        assert_eq!(
            initial_config_dirs(None, &claude),
            None,
            "~/.claude がなければ決めずに次の起動でまた探す"
        );
        std::fs::create_dir(&claude).unwrap();
        assert_eq!(
            initial_config_dirs(None, &claude),
            Some(vec![claude.to_string_lossy().into_owned()])
        );
        assert_eq!(
            initial_config_dirs(Some(&[]), &claude),
            None,
            "すべて削除した後は入れ直さない"
        );
        assert_eq!(initial_config_dirs(Some(&["/x".into()]), &claude), None);
    }

    #[test]
    fn monitors_only_listed_dirs_after_normalizing_paths() {
        let home = tempfile::tempdir().unwrap();
        let real = home.path().join(".claude");
        std::fs::create_dir(&real).unwrap();
        let link = home.path().join("link-to-claude");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let monitored = Monitored::new(&[format!("{}/", real.display())]);
        assert!(monitored.includes("s1", &real.to_string_lossy()));
        assert!(
            monitored.includes("s1", &link.to_string_lossy()),
            "シンボリックリンクは実体で照らす"
        );
        assert!(!monitored.includes("s1", "/Users/u/.claude-work"));
        assert!(
            monitored.includes("pid:42", ""),
            "状態不明の行はどの設定か分からないので出す"
        );
        assert!(!Monitored::new(&[]).includes("s1", &real.to_string_lossy()));
    }

    #[test]
    fn shows_home_as_tilde() {
        let home = Path::new("/Users/u");
        assert_eq!(display_path("/Users/u/.claude", home), "~/.claude");
        assert_eq!(display_path("/Users/u", home), "~");
        assert_eq!(display_path("/Users/uu/.claude", home), "/Users/uu/.claude");
        assert_eq!(display_path("/opt/claude", home), "/opt/claude");
    }

    #[test]
    fn offers_codex_dir_once() {
        let home = tempfile::tempdir().unwrap();
        let codex = home.path().join(".codex");
        let claude = "/u/.claude".to_string();
        assert_eq!(
            offer_codex_dir(Some(std::slice::from_ref(&claude)), false, &codex),
            None,
            "~/.codex がなければ足さない"
        );
        std::fs::create_dir(&codex).unwrap();
        let codex_s = codex.to_string_lossy().into_owned();
        assert_eq!(
            offer_codex_dir(Some(std::slice::from_ref(&claude)), false, &codex),
            Some(vec![claude.clone(), codex_s.clone()])
        );
        assert_eq!(
            offer_codex_dir(None, false, &codex),
            Some(vec![codex_s.clone()]),
            "まだ決めていなくても足す"
        );
        assert_eq!(
            offer_codex_dir(Some(std::slice::from_ref(&claude)), true, &codex),
            None,
            "外した後は足さない"
        );
        assert_eq!(
            offer_codex_dir(Some(std::slice::from_ref(&codex_s)), false, &codex),
            None,
            "もう入っている"
        );
    }

    #[test]
    fn codex_dir_already_listed_in_another_form_is_not_offered_again() {
        let home = tempfile::tempdir().unwrap();
        let codex = home.path().join(".codex");
        std::fs::create_dir(&codex).unwrap();
        let link = home.path().join("codex-link");
        std::os::unix::fs::symlink(&codex, &link).unwrap();
        let with_slash = format!("{}/", codex.to_string_lossy());
        assert_eq!(
            offer_codex_dir(Some(std::slice::from_ref(&with_slash)), false, &codex),
            None
        );
        let via_link = link.to_string_lossy().into_owned();
        assert_eq!(
            offer_codex_dir(Some(std::slice::from_ref(&via_link)), false, &codex),
            None
        );
    }

    #[test]
    fn a_folder_counts_as_listed_however_it_is_written() {
        let home = tempfile::tempdir().unwrap();
        let claude = home.path().join(".claude");
        std::fs::create_dir(&claude).unwrap();
        let link = home.path().join("claude-link");
        std::os::unix::fs::symlink(&claude, &link).unwrap();
        let listed = vec![claude.to_string_lossy().into_owned()];
        assert!(lists_dir(&listed, &format!("{}/", claude.to_string_lossy())));
        assert!(lists_dir(&listed, &link.to_string_lossy()));
        assert!(!lists_dir(&listed, "/u/.codex"));
    }

    #[test]
    fn choosing_the_codex_dir_by_hand_counts_however_it_is_written() {
        let home = tempfile::tempdir().unwrap();
        let codex = home.path().join(".codex");
        std::fs::create_dir(&codex).unwrap();
        let link = home.path().join("codex-link");
        std::os::unix::fs::symlink(&codex, &link).unwrap();
        let codex_s = codex.to_string_lossy().into_owned();
        assert!(is_codex_dir(&codex_s, &codex));
        assert!(is_codex_dir(&format!("{codex_s}/"), &codex));
        assert!(is_codex_dir(&link.to_string_lossy(), &codex));
        assert!(!is_codex_dir("/u/.claude", &codex));
    }
}
