use std::fs;
use std::path::Path;

// 起動したときに作っておく。作らないと、最初に開くときに WebKit のプロセスを 3 つとも起動するので、ログインして最初の 1 回だけ開くのが遅い。
// 閉じたら捨てるので、一度開いて閉じた後のメモリーはどちらでも同じ（2026-10-12 にユーザーと決定）
#[test]
fn list_window_is_created_at_startup() {
    let conf: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json")).unwrap(),
    )
    .unwrap();
    let list = conf["app"]["windows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["label"] == "list")
        .expect("list のウィンドウの設定がない");
    assert_ne!(list["create"], false);
}

// 一覧は開くたびに作り直すので、スクリプトが描き直す前の HTML の状態が毎回一瞬見える。ふだん最初に見える状態にしておく
#[test]
fn list_starts_on_the_agents_tab_without_unknown() {
    let html = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../index.html")).unwrap();
    let tab = |page: &str| {
        html.lines()
            .find(|line| line.contains(&format!("data-page=\"{page}\"")))
            .unwrap_or_else(|| panic!("{page} のタブがない"))
            .to_string()
    };
    assert!(tab("unknown").contains(" hidden"), "Unknown のタブは隠しておく");
    assert!(
        tab("agents").contains("class=\"tab selected\""),
        "Agents のタブを選んでおく"
    );
}

// 公開のビルドが更新を確かめに行く先と、作る成果物。macOS 14 より前では起動させない
#[test]
fn release_settings_are_present() {
    let conf: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(conf["bundle"]["macOS"]["minimumSystemVersion"], "14.0");
    assert_eq!(conf["bundle"]["createUpdaterArtifacts"], true);
    assert_eq!(
        conf["plugins"]["updater"]["endpoints"][0],
        "https://github.com/imatomix/agent-loaf/releases/latest/download/latest.json"
    );
    // 入れ替える前に中身を確かめる鍵。ないと公開のビルドが起動できず、手元のビルドも止まる
    assert!(
        conf["plugins"]["updater"]["pubkey"]
            .as_str()
            .is_some_and(|key| key.len() > 40),
        "自動更新の公開鍵を入れること"
    );
}
