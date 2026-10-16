use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub fn sync_marketplace(source: &Path, dest: &Path) -> io::Result<bool> {
    let wanted = files_under(source)?;
    let mut changed = false;
    for relative in &wanted {
        let bytes = fs::read(source.join(relative))?;
        let target = dest.join(relative);
        if fs::read(&target).ok().as_deref() == Some(bytes.as_slice()) {
            continue;
        }
        fs::create_dir_all(target.parent().unwrap_or(dest))?;
        fs::write(&target, bytes)?;
        changed = true;
    }
    if dest.exists() {
        let keep: HashSet<&PathBuf> = wanted.iter().collect();
        for relative in files_under(dest)? {
            if !keep.contains(&relative) {
                fs::remove_file(dest.join(&relative))?;
                changed = true;
            }
        }
    }
    Ok(changed)
}

pub fn bundled_plugin_version(dest: &Path) -> Option<String> {
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(dest.join("plugin/.claude-plugin/plugin.json")).ok()?).ok()?;
    manifest["version"].as_str().map(String::from)
}

fn files_under(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    let mut pending = vec![PathBuf::new()];
    while let Some(relative) = pending.pop() {
        for entry in fs::read_dir(root.join(&relative))? {
            let entry = entry?;
            let path = relative.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                pending.push(path);
            } else {
                found.push(path);
            }
        }
    }
    found.sort();
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, body: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }

    #[test]
    fn copies_tree_and_reports_changes_only_when_content_differs() {
        let src = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        write(&src.path().join(".claude-plugin/marketplace.json"), "{}");
        write(
            &src.path().join("plugin/.claude-plugin/plugin.json"),
            r#"{"version":"0.2.0"}"#,
        );
        write(&src.path().join("plugin/hooks/hooks.json"), "{}");
        let out = dest.path().join("marketplace");
        assert!(sync_marketplace(src.path(), &out).unwrap());
        assert_eq!(fs::read_to_string(out.join("plugin/hooks/hooks.json")).unwrap(), "{}");
        assert!(!sync_marketplace(src.path(), &out).unwrap(), "同じ中身なら写し直さない");
        write(&src.path().join("plugin/hooks/hooks.json"), r#"{"a":1}"#);
        assert!(sync_marketplace(src.path(), &out).unwrap());
        assert_eq!(bundled_plugin_version(&out).as_deref(), Some("0.2.0"));
    }

    #[test]
    fn removes_files_that_no_longer_exist_in_the_bundle() {
        let src = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        write(&src.path().join("plugin/a.json"), "{}");
        let out = dest.path().join("marketplace");
        write(&out.join("plugin/stale.json"), "{}");
        assert!(sync_marketplace(src.path(), &out).unwrap());
        assert!(!out.join("plugin/stale.json").exists());
    }

    #[test]
    fn missing_source_is_an_error() {
        let dest = tempfile::tempdir().unwrap();
        assert!(sync_marketplace(Path::new("/nonexistent/src"), &dest.path().join("m")).is_err());
    }
}
