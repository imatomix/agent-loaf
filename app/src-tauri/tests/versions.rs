use std::fs;
use std::path::Path;

fn json_version(path: &Path) -> String {
    let value: serde_json::Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    value["version"]
        .as_str()
        .unwrap_or_else(|| panic!("{} に version がない", path.display()))
        .to_string()
}

fn inherits_workspace_version(path: &Path) -> bool {
    package_inherits_workspace_version(&fs::read_to_string(path).unwrap())
}

// toml のライブラリを足さないため、見出しの行で [package] の中かどうかを追う
fn package_inherits_workspace_version(cargo_toml: &str) -> bool {
    let mut in_package = false;
    cargo_toml.lines().map(str::trim).any(|line| {
        if line.starts_with('[') {
            in_package = line == "[package]";
            return false;
        }
        in_package && line == "version.workspace = true"
    })
}

#[test]
fn workspace_version_counts_only_inside_the_package_section() {
    assert!(package_inherits_workspace_version(
        "[package]\nname = \"a\"\nversion.workspace = true\n\n[dependencies]\nserde = \"1\"\n"
    ));
    assert!(!package_inherits_workspace_version(
        "[package]\nname = \"a\"\nversion = \"0.1.0\"\n\n[dependencies.b]\nversion.workspace = true\n"
    ));
}

// プラグインの版が変わらないと、入れてあるプラグインは更新されず、hooks.json の変更が利用者に届かない
#[test]
fn every_version_matches_the_app() {
    let app = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = app.join("../..");
    let expected = env!("CARGO_PKG_VERSION");
    for file in ["app/package.json", "plugin/.claude-plugin/plugin.json"] {
        assert_eq!(
            json_version(&repo.join(file)),
            expected,
            "{file} の版をアプリ（{expected}）に合わせること"
        );
    }
    for file in [
        "crates/core/Cargo.toml",
        "crates/hook/Cargo.toml",
        "app/src-tauri/Cargo.toml",
    ] {
        assert!(
            inherits_workspace_version(&repo.join(file)),
            "{file} は version.workspace = true でルートの版を使うこと"
        );
    }
    // Tauri は tauri.conf.json に版がないと Cargo.toml の版を使う。書くと 2 か所になる
    let conf: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(app.join("tauri.conf.json")).unwrap()).unwrap();
    assert!(
        conf.get("version").is_none(),
        "tauri.conf.json に version を書かないこと"
    );
}
