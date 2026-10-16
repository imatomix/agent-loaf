use std::fs;
use std::path::Path;
use std::time::Duration;

use agent_loaf_core::claude_cli::{find_claude, is_executable, ClaudeCli, CLI_TIMEOUT};
use agent_loaf_core::install_plan::{install_steps, remove_steps, INSPECT_MARKETPLACES, INSPECT_PLUGINS};
use agent_loaf_core::marketplace::sync_marketplace;
use agent_loaf_core::plugin_status::{parse_inspection, DirState, Inspection};

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|w| w.to_string()).collect()
}

fn inspect(cli: &ClaudeCli, config: &Path, bundled: &Path) -> Inspection {
    let plugins = cli.run(config, &words(&INSPECT_PLUGINS), CLI_TIMEOUT).unwrap();
    let marketplaces = cli.run(config, &words(&INSPECT_MARKETPLACES), CLI_TIMEOUT).unwrap();
    parse_inspection(&plugins, &marketplaces, bundled).unwrap()
}

#[test]
#[ignore = "本物の claude を使う。scripts/integration-test.sh から動かす"]
fn installs_receives_events_and_removes() {
    let home = dirs::home_dir().unwrap();
    let cli = ClaudeCli::new(find_claude(None, &home, is_executable).expect("claude が見つからない"));
    let work = tempfile::tempdir().unwrap();
    let config = work.path().join("config");
    let monitor_home = work.path().join("monitor");
    let bundled = monitor_home.join("marketplace");
    fs::create_dir_all(&config).unwrap();
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    fs::create_dir_all(bundled.join(".claude-plugin")).unwrap();
    fs::copy(
        repo.join(".claude-plugin/marketplace.json"),
        bundled.join(".claude-plugin/marketplace.json"),
    )
    .unwrap();
    sync_marketplace(&repo.join("plugin"), &bundled.join("plugin")).unwrap();

    let before = inspect(&cli, &config, &bundled);
    assert_eq!(before.state(), DirState::NotInstalled);
    for step in install_steps(&before) {
        cli.run(&config, &step.args(&bundled), CLI_TIMEOUT).unwrap();
    }
    let installed = inspect(&cli, &config, &bundled);
    assert_eq!(installed.state(), DirState::Installed);

    if std::env::var_os("ANTHROPIC_API_KEY").is_some() || std::env::var_os("CLAUDE_CODE_OAUTH_TOKEN").is_some() {
        std::env::set_var("AGENT_LOAF_HOME", &monitor_home);
        cli.run(
            &config,
            &words(&["-p", "「はい」とだけ答えて"]),
            Duration::from_secs(120),
        )
        .unwrap();
        let events = fs::read_dir(monitor_home.join("events"))
            .map(|d| d.count())
            .unwrap_or(0);
        assert!(
            events > 0,
            "イベントファイルが届いていない（~/.agent-loaf/bin/agent-loaf-hook があるか確かめる）"
        );
    } else {
        eprintln!("ANTHROPIC_API_KEY も CLAUDE_CODE_OAUTH_TOKEN もないので、claude -p でのイベントの確認は飛ばします");
    }

    for step in remove_steps(&installed) {
        cli.run(&config, &step.args(&bundled), CLI_TIMEOUT).unwrap();
    }
    assert_eq!(inspect(&cli, &config, &bundled).state(), DirState::NotInstalled);
}
