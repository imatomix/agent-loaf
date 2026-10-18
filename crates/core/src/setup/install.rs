use std::fs;
use std::io;
use std::path::Path;

pub fn install_hook_binary(source: &Path, dest: &Path) -> io::Result<bool> {
    let new_bytes = fs::read(source)?;
    if fs::read(dest).ok().as_deref() == Some(new_bytes.as_slice()) {
        return Ok(false);
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    // hook が実行中でも安全に差し替えられるよう、別名で書いてから置き換える
    let tmp = dest.with_extension("tmp");
    fs::write(&tmp, &new_bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))?;
    }
    fs::rename(&tmp, dest)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_executable_and_skips_when_identical() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("src-hook");
        fs::write(&source, b"v1").unwrap();
        let dest = dir.path().join("home/bin/agent-loaf-hook");
        assert!(install_hook_binary(&source, &dest).unwrap());
        assert_eq!(fs::read(&dest).unwrap(), b"v1");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&dest).unwrap().permissions().mode() & 0o777, 0o755);
        }
        assert!(!install_hook_binary(&source, &dest).unwrap());
        fs::write(&source, b"v2").unwrap();
        assert!(install_hook_binary(&source, &dest).unwrap());
        assert_eq!(fs::read(&dest).unwrap(), b"v2");
    }

    #[test]
    fn missing_source_is_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(install_hook_binary(&dir.path().join("none"), &dir.path().join("dest")).is_err());
    }
}
