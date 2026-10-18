use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::session::event::MonitorEvent;

pub const MAX_SPOOL_FILES: usize = 10_000;
// hook は 5 秒で打ち切られるので、1 分たった一時ファイルが書き終わることはない
pub const STALE_TMP_AGE: std::time::Duration = std::time::Duration::from_secs(60);

#[derive(Debug, Default)]
pub struct SpoolBatch {
    pub events: Vec<MonitorEvent>,
    pub skipped: usize,
}

// アプリが止まっていても hook は動き続けるので、上限を超えたら書くのをやめて ~/.agent-loaf が際限なく大きくならないようにする
pub fn spool_is_full(events_dir: &Path, max_files: usize) -> bool {
    fs::read_dir(events_dir).is_ok_and(|entries| entries.take(max_files).count() >= max_files)
}

pub fn write_event(events_dir: &Path, event: &MonitorEvent) -> io::Result<PathBuf> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let name = format!("{:013}-{}-{:09}.json", event.ts_ms, std::process::id(), nanos);
    let final_path = events_dir.join(&name);
    // 途中まで書かれたファイルをアプリに読ませないため、一時ファイルに書いてから名前を変える
    let tmp_path = events_dir.join(format!("{name}.tmp"));
    crate::base::paths::write_private(&tmp_path, &serde_json::to_vec(event)?)?;
    fs::rename(&tmp_path, &final_path)?;
    Ok(final_path)
}

pub fn drain_spool(events_dir: &Path, max_files: usize) -> SpoolBatch {
    let Ok(entries) = fs::read_dir(events_dir) else {
        return SpoolBatch::default();
    };
    let now = SystemTime::now();
    let mut paths: Vec<PathBuf> = Vec::new();
    for path in entries.filter_map(|entry| entry.ok().map(|e| e.path())) {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("json") => paths.push(path),
            // hook が書いている途中で打ち切られると、一時ファイルだけが残る
            Some("tmp") if is_stale(&path, now) => {
                let _ = fs::remove_file(&path);
            }
            _ => {}
        }
    }
    paths.sort();

    let mut batch = SpoolBatch::default();
    let overflow = paths.len().saturating_sub(max_files);
    for path in paths.drain(..overflow) {
        let _ = fs::remove_file(path);
        batch.skipped += 1;
    }
    for path in paths {
        match fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<MonitorEvent>(&bytes).ok())
        {
            Some(event) => batch.events.push(event),
            None => batch.skipped += 1,
        }
        let _ = fs::remove_file(&path);
    }
    batch.events.sort_by_key(|e| e.ts_ms);
    batch
}

fn is_stale(path: &Path, now: SystemTime) -> bool {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .is_ok_and(|modified| now.duration_since(modified).is_ok_and(|age| age >= STALE_TMP_AGE))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::event::EventDetail;

    fn ev(event: &str, ts_ms: u64) -> MonitorEvent {
        MonitorEvent {
            schema: 1,
            ts_ms,
            event: event.into(),
            session_id: "s1".into(),
            agent_id: None,
            claude_pid: None,
            config_dir: "/c".into(),
            cwd: "/w".into(),
            transcript_path: None,
            detail: EventDetail::default(),
            terminal: None,
            git_branch: None,
            agent: Default::default(),
        }
    }

    #[test]
    fn writes_and_drains_in_time_order() {
        let dir = tempfile::tempdir().unwrap();
        write_event(dir.path(), &ev("Stop", 3_000)).unwrap();
        write_event(dir.path(), &ev("SessionStart", 1_000)).unwrap();
        write_event(dir.path(), &ev("UserPromptSubmit", 2_000)).unwrap();
        let batch = drain_spool(dir.path(), MAX_SPOOL_FILES);
        let names: Vec<_> = batch.events.iter().map(|e| e.event.as_str()).collect();
        assert_eq!(names, ["SessionStart", "UserPromptSubmit", "Stop"]);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn leaves_no_tmp_file_after_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_event(dir.path(), &ev("Stop", 1)).unwrap();
        assert_eq!(path.extension().unwrap(), "json");
        let names: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn skips_broken_files_and_ignores_tmp() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("0000000000001-1-1.json"), "{broken").unwrap();
        fs::write(dir.path().join("0000000000002-1-1.json.tmp"), "{}").unwrap();
        write_event(dir.path(), &ev("Stop", 5)).unwrap();
        let batch = drain_spool(dir.path(), MAX_SPOOL_FILES);
        assert_eq!(batch.events.len(), 1);
        assert_eq!(batch.skipped, 1);
        assert!(dir.path().join("0000000000002-1-1.json.tmp").exists());
    }

    #[test]
    fn removes_tmp_files_left_behind_by_killed_hooks() {
        let dir = tempfile::tempdir().unwrap();
        let fresh = dir.path().join("0000000000002-1-1.json.tmp");
        let stale = dir.path().join("0000000000001-1-1.json.tmp");
        fs::write(&fresh, "{}").unwrap();
        fs::write(&stale, "{}").unwrap();
        let old = SystemTime::now() - STALE_TMP_AGE - std::time::Duration::from_secs(1);
        fs::File::options()
            .write(true)
            .open(&stale)
            .unwrap()
            .set_modified(old)
            .unwrap();
        drain_spool(dir.path(), MAX_SPOOL_FILES);
        assert!(fresh.exists(), "書いている途中かもしれないので消さない");
        assert!(!stale.exists());
    }

    #[test]
    fn drops_oldest_files_beyond_limit() {
        let dir = tempfile::tempdir().unwrap();
        for ts in 1..=5 {
            write_event(dir.path(), &ev("Stop", ts)).unwrap();
        }
        let batch = drain_spool(dir.path(), 2);
        let ts: Vec<_> = batch.events.iter().map(|e| e.ts_ms).collect();
        assert_eq!(ts, [4, 5]);
        assert_eq!(batch.skipped, 3);
    }

    #[test]
    fn reports_full_only_at_the_limit() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!spool_is_full(dir.path(), 2));
        write_event(dir.path(), &ev("Stop", 1)).unwrap();
        assert!(!spool_is_full(dir.path(), 2));
        write_event(dir.path(), &ev("Stop", 2)).unwrap();
        assert!(spool_is_full(dir.path(), 2));
        drain_spool(dir.path(), MAX_SPOOL_FILES);
        assert!(!spool_is_full(dir.path(), 2), "アプリが読み取ったらまた書ける");
        assert!(!spool_is_full(Path::new("/nonexistent/agent-loaf"), 2));
    }

    #[test]
    fn missing_directory_yields_empty_batch() {
        let batch = drain_spool(Path::new("/nonexistent/agent-loaf"), 10);
        assert!(batch.events.is_empty());
    }

    #[test]
    fn events_from_older_hooks_are_claude_code() {
        let old = r#"{"schema":1,"ts_ms":1,"event":"Stop","session_id":"s1","agent_id":null,"claude_pid":7,"config_dir":"/c","cwd":"/w","transcript_path":null,"detail":{"tool_name":null,"summary":null,"notification_type":null,"error_type":null},"terminal":null,"git_branch":null}"#;
        let event: MonitorEvent = serde_json::from_str(old).unwrap();
        assert_eq!(event.agent, crate::base::agent::Agent::ClaudeCode);
    }
}
