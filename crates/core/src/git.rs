use std::fs;
use std::path::{Path, PathBuf};

pub fn git_branch(cwd: &Path) -> Option<String> {
    let git_dir = find_git_dir(cwd)?;
    let head = fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    match head.strip_prefix("ref: refs/heads/") {
        Some(branch) => Some(branch.to_string()),
        None => Some(head.chars().take(7).collect()),
    }
}

fn find_git_dir(start: &Path) -> Option<PathBuf> {
    for dir in start.ancestors() {
        let dot_git = dir.join(".git");
        if dot_git.is_dir() {
            return Some(dot_git);
        }
        if dot_git.is_file() {
            let content = fs::read_to_string(&dot_git).ok()?;
            let target = Path::new(content.trim().strip_prefix("gitdir: ")?);
            return Some(if target.is_absolute() {
                target.to_path_buf()
            } else {
                dir.join(target)
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_branch_from_head() {
        let repo = tempfile::tempdir().unwrap();
        fs::create_dir_all(repo.path().join(".git")).unwrap();
        fs::write(repo.path().join(".git/HEAD"), "ref: refs/heads/feature/x\n").unwrap();
        let sub = repo.path().join("src/deep");
        fs::create_dir_all(&sub).unwrap();
        assert_eq!(git_branch(&sub).as_deref(), Some("feature/x"));
    }

    #[test]
    fn follows_worktree_gitdir_file() {
        let root = tempfile::tempdir().unwrap();
        let real = root.path().join("main/.git/worktrees/wt");
        fs::create_dir_all(&real).unwrap();
        fs::write(real.join("HEAD"), "ref: refs/heads/wt-branch\n").unwrap();
        let wt = root.path().join("wt");
        fs::create_dir_all(&wt).unwrap();
        fs::write(wt.join(".git"), format!("gitdir: {}\n", real.display())).unwrap();
        assert_eq!(git_branch(&wt).as_deref(), Some("wt-branch"));
    }

    #[test]
    fn shows_short_hash_when_detached() {
        let repo = tempfile::tempdir().unwrap();
        fs::create_dir_all(repo.path().join(".git")).unwrap();
        fs::write(repo.path().join(".git/HEAD"), "0123456789abcdef\n").unwrap();
        assert_eq!(git_branch(repo.path()).as_deref(), Some("0123456"));
    }

    #[test]
    fn returns_none_outside_repo() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(git_branch(dir.path()), None);
    }
}
