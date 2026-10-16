use std::fs;
use std::path::Path;

use agent_loaf_core::event::MonitorEvent;
use agent_loaf_core::state::Status;
use agent_loaf_core::store::SessionStore;

fn parse_status(name: &str) -> Option<Status> {
    match name {
        "needs_action" => Some(Status::NeedsAction),
        "error" => Some(Status::Error),
        "done" => Some(Status::Done),
        "running" => Some(Status::Running),
        "ended" => Some(Status::Ended),
        _ => None,
    }
}

#[test]
fn recorded_sessions_reach_expected_status() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut checked = 0;
    for entry in fs::read_dir(&dir).expect("Task 6 のテストデータがあること") {
        let path = entry.unwrap().path();
        let file_name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let Some((expected, _)) = file_name.split_once("__") else {
            continue;
        };
        let expected = parse_status(expected).unwrap_or_else(|| panic!("{file_name}: 不明な状態名"));

        let mut store = SessionStore::default();
        let mut main_session = None;
        for line in fs::read_to_string(&path)
            .unwrap()
            .lines()
            .filter(|l| !l.trim().is_empty())
        {
            let event: MonitorEvent = serde_json::from_str(line).unwrap();
            main_session.get_or_insert_with(|| event.session_id.clone());
            store.apply(&event);
        }
        let main_session = main_session.unwrap_or_else(|| panic!("{file_name}: イベントがない"));
        let actual = store.sessions().iter().find(|s| s.id == main_session).unwrap().status;
        assert_eq!(actual, expected, "{file_name}");
        checked += 1;
    }
    assert!(checked > 0, "テストデータが 1 件もない");
}
