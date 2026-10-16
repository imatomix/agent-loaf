use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

// Task 6 の調査で会話ログに残ると確認した中断の目印（「… for tool use]」の形もある）
pub const INTERRUPT_MARKER: &str = "[Request interrupted by user";
const TAIL_BYTES: u64 = 64 * 1024;

pub fn transcript_ends_with_interrupt(path: &Path) -> bool {
    let Ok(mut file) = File::open(path) else { return false };
    let Ok(len) = file.metadata().map(|m| m.len()) else {
        return false;
    };
    if file.seek(SeekFrom::Start(len.saturating_sub(TAIL_BYTES))).is_err() {
        return false;
    }
    let mut tail = Vec::new();
    if file.read_to_end(&mut tail).is_err() {
        return false;
    }
    // 中断の目印の後ろには管理用の行（type が system や mode など）が続くため、最後のメッセージ行を探す
    String::from_utf8_lossy(&tail)
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(is_message)
        .is_some_and(|message| is_interrupt(&message))
}

fn is_message(value: &serde_json::Value) -> bool {
    matches!(value.get("type").and_then(|t| t.as_str()), Some("user" | "assistant"))
}

// ツールの結果（tool_result）も type が user の行に入る。ファイルの中身などに目印の文字が含まれることがあるので、利用者の入力の文の先頭だけを見る
fn is_interrupt(message: &serde_json::Value) -> bool {
    if message.get("type").and_then(|t| t.as_str()) != Some("user") {
        return false;
    }
    let starts_with_marker = |text: &str| text.starts_with(INTERRUPT_MARKER);
    match message.pointer("/message/content") {
        Some(serde_json::Value::String(text)) => starts_with_marker(text),
        Some(serde_json::Value::Array(blocks)) => blocks.iter().any(|block| {
            block.get("type").and_then(|t| t.as_str()) == Some("text")
                && block
                    .get("text")
                    .and_then(|t| t.as_str())
                    .is_some_and(starts_with_marker)
        }),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(lines: &[&str]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.jsonl");
        fs::write(&path, lines.join("\n") + "\n").unwrap();
        (dir, path)
    }

    #[test]
    fn detects_marker_followed_by_bookkeeping_lines() {
        let (_dir, path) = write(&[
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"1\n2"}]}}"#,
            r#"{"type":"user","message":{"content":[{"type":"text","text":"[Request interrupted by user]"}]}}"#,
            r#"{"type":"system","subtype":"turn_duration"}"#,
            r#"{"type":"mode"}"#,
            r#"{"type":"permission-mode"}"#,
        ]);
        assert!(transcript_ends_with_interrupt(&path));
    }

    #[test]
    fn detects_tool_use_rejection_marker() {
        let (_dir, path) = write(&[
            r#"{"type":"user","message":{"content":[{"type":"text","text":"[Request interrupted by user for tool use]"}]}}"#,
            r#"{"type":"system","subtype":"turn_duration"}"#,
        ]);
        assert!(transcript_ends_with_interrupt(&path));
    }

    #[test]
    fn ignores_marker_before_later_messages() {
        let (_dir, path) = write(&[
            r#"{"type":"user","message":{"content":[{"type":"text","text":"[Request interrupted by user]"}]}}"#,
            r#"{"type":"user","message":{"content":"次の指示"}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"了解"}]}}"#,
        ]);
        assert!(!transcript_ends_with_interrupt(&path));
    }

    #[test]
    fn ignores_marker_inside_tool_output() {
        let (_dir, path) = write(&[
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":"Read","input":{}}]}}"#,
            r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":"pub const INTERRUPT_MARKER: &str = \"[Request interrupted by user\";"}]}}"#,
        ]);
        assert!(!transcript_ends_with_interrupt(&path));
    }

    #[test]
    fn ignores_marker_in_the_middle_of_a_prompt() {
        let (_dir, path) =
            write(&[r#"{"type":"user","message":{"content":"ログに [Request interrupted by user] と出る理由は？"}}"#]);
        assert!(!transcript_ends_with_interrupt(&path));
    }

    #[test]
    fn detects_marker_given_as_plain_string_content() {
        let (_dir, path) = write(&[r#"{"type":"user","message":{"content":"[Request interrupted by user]"}}"#]);
        assert!(transcript_ends_with_interrupt(&path));
    }

    #[test]
    fn detects_real_interrupt_after_tool_output_with_marker() {
        let (_dir, path) = write(&[
            r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":"[Request interrupted by user"}]}}"#,
            r#"{"type":"user","message":{"content":[{"type":"text","text":"[Request interrupted by user for tool use]"}]}}"#,
            r#"{"type":"system","subtype":"turn_duration"}"#,
        ]);
        assert!(transcript_ends_with_interrupt(&path));
    }

    #[test]
    fn missing_file_is_false() {
        assert!(!transcript_ends_with_interrupt(Path::new("/nonexistent/t.jsonl")));
    }
}
