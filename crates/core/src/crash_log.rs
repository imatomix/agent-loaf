use std::fs;
use std::io;
use std::path::Path;

use crate::paths::write_private;

pub const MAX_BYTES: usize = 64 * 1024;

// panic のときにだけ書く。メニューバーのアプリの標準エラーはどこにも残らないので、ここに残して原因を追えるようにする
pub fn append(path: &Path, now_ms: u64, message: &str) -> io::Result<()> {
    let mut text = fs::read_to_string(path).unwrap_or_default();
    text.push_str(&format!("{now_ms} {message}\n"));
    if text.len() > MAX_BYTES {
        let cut = text.len() - MAX_BYTES;
        let start = text[cut..].find('\n').map_or(text.len(), |i| cut + i + 1);
        text.drain(..start);
    }
    write_private(path, text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_lines_and_trims_the_oldest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("crash.log");
        append(&path, 1, "first").unwrap();
        append(&path, 2, "second").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "1 first\n2 second\n");
        let long = "x".repeat(MAX_BYTES / 3);
        for ts in 3..10 {
            append(&path, ts, &long).unwrap();
        }
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.len() <= MAX_BYTES);
        assert!(text.ends_with(&format!("9 {long}\n")));
        assert!(!text.contains("first"));
        assert!(text.starts_with(|c: char| c.is_ascii_digit()), "行の途中から始めない");
    }
}
