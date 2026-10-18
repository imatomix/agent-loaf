use std::path::Path;

use crate::base::agent::Agent;
use crate::setup::plugin_status::{Inspection, MarketplacePresence, PluginPresence, MARKETPLACE_NAME, PLUGIN_ID};

pub const INSPECT_PLUGINS: [&str; 3] = ["plugin", "list", "--json"];
pub const INSPECT_MARKETPLACES: [&str; 4] = ["plugin", "marketplace", "list", "--json"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliStep {
    MarketplaceAdd,
    MarketplaceRemove,
    Install,
    Update,
    Uninstall,
    Enable,
}

impl CliStep {
    // 写しに失敗していると古い中身や空のフォルダーから入れてしまうので、入れる側の手順だけを止める。外す側は写しを使わない
    pub fn needs_bundled_copy(&self) -> bool {
        matches!(self, CliStep::MarketplaceAdd | CliStep::Install | CliStep::Update)
    }

    pub fn args(&self, bundled_dir: &Path) -> Vec<String> {
        self.args_for(Agent::ClaudeCode, bundled_dir)
    }

    pub fn args_for(&self, agent: Agent, bundled_dir: &Path) -> Vec<String> {
        if *self == CliStep::MarketplaceAdd {
            return vec![
                "plugin".into(),
                "marketplace".into(),
                "add".into(),
                bundled_dir.to_string_lossy().into_owned(),
            ];
        }
        let words: Vec<&str> = match (agent, self) {
            (_, CliStep::MarketplaceRemove) => vec!["plugin", "marketplace", "remove", MARKETPLACE_NAME],
            (Agent::ClaudeCode, CliStep::Install) => vec!["plugin", "install", PLUGIN_ID],
            (Agent::ClaudeCode, CliStep::Update) => vec!["plugin", "update", PLUGIN_ID],
            (Agent::ClaudeCode, CliStep::Uninstall) => vec!["plugin", "uninstall", PLUGIN_ID],
            (Agent::ClaudeCode, CliStep::Enable) => vec!["plugin", "enable", PLUGIN_ID],
            // Codex には update と enable がなく、もう一度 add すると新しい版で入れ直し、有効にする
            (Agent::Codex, CliStep::Install | CliStep::Update | CliStep::Enable) => vec!["plugin", "add", PLUGIN_ID],
            (Agent::Codex, CliStep::Uninstall) => vec!["plugin", "remove", PLUGIN_ID],
            (_, CliStep::MarketplaceAdd) => unreachable!("先に返している"),
        };
        words.into_iter().map(String::from).collect()
    }
}

pub fn install_steps(inspection: &Inspection) -> Vec<CliStep> {
    match (&inspection.plugin, inspection.marketplace) {
        (PluginPresence::Enabled { .. }, _) => vec![],
        (PluginPresence::Disabled, _) => vec![CliStep::Enable],
        (PluginPresence::Absent, MarketplacePresence::Absent) => vec![CliStep::MarketplaceAdd, CliStep::Install],
        (PluginPresence::Absent, _) => vec![CliStep::Install],
    }
}

// フォルダーのマーケットプレイスは中身を読み直すので、marketplace update をしなくても plugin update で新しい版になる（2026-10-11 に確認）
pub fn update_steps(inspection: &Inspection, bundled_version: &str) -> Vec<CliStep> {
    match (&inspection.plugin, inspection.marketplace) {
        (PluginPresence::Enabled { version }, MarketplacePresence::Bundled) if version != bundled_version => {
            vec![CliStep::Update]
        }
        _ => vec![],
    }
}

pub fn remove_steps(inspection: &Inspection) -> Vec<CliStep> {
    let mut steps = Vec::new();
    if inspection.plugin != PluginPresence::Absent {
        steps.push(CliStep::Uninstall);
    }
    if inspection.marketplace == MarketplacePresence::Bundled {
        steps.push(CliStep::MarketplaceRemove);
    }
    steps
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::plugin_status::{MarketplacePresence as M, PluginPresence as P};

    fn i(plugin: P, marketplace: M) -> Inspection {
        Inspection { plugin, marketplace }
    }
    fn enabled(v: &str) -> P {
        P::Enabled { version: v.into() }
    }

    #[test]
    fn install_registers_marketplace_only_when_missing() {
        assert_eq!(
            install_steps(&i(P::Absent, M::Absent)),
            [CliStep::MarketplaceAdd, CliStep::Install]
        );
        assert_eq!(install_steps(&i(P::Absent, M::Bundled)), [CliStep::Install]);
        assert_eq!(
            install_steps(&i(P::Absent, M::Other)),
            [CliStep::Install],
            "別の場所のマーケットプレイスから入れる"
        );
        assert_eq!(install_steps(&i(P::Disabled, M::Other)), [CliStep::Enable]);
        assert!(install_steps(&i(enabled("0.1.0"), M::Bundled)).is_empty());
    }

    #[test]
    fn update_only_bundled_installs_with_a_different_version() {
        assert_eq!(
            update_steps(&i(enabled("0.1.0"), M::Bundled), "0.2.0"),
            [CliStep::Update]
        );
        assert!(update_steps(&i(enabled("0.2.0"), M::Bundled), "0.2.0").is_empty());
        assert!(
            update_steps(&i(enabled("0.1.0"), M::Other), "0.2.0").is_empty(),
            "別の場所のものはその元で更新される"
        );
        assert!(update_steps(&i(P::Absent, M::Bundled), "0.2.0").is_empty());
    }

    #[test]
    fn remove_uninstalls_and_drops_only_our_marketplace() {
        assert_eq!(
            remove_steps(&i(enabled("0.1.0"), M::Bundled)),
            [CliStep::Uninstall, CliStep::MarketplaceRemove]
        );
        assert_eq!(remove_steps(&i(enabled("0.1.0"), M::Other)), [CliStep::Uninstall]);
        assert_eq!(
            remove_steps(&i(P::Disabled, M::Bundled)),
            [CliStep::Uninstall, CliStep::MarketplaceRemove]
        );
        assert_eq!(remove_steps(&i(P::Absent, M::Bundled)), [CliStep::MarketplaceRemove]);
        assert!(remove_steps(&i(P::Absent, M::Other)).is_empty());
    }

    #[test]
    fn only_putting_the_plugin_in_needs_the_bundled_copy() {
        for step in [CliStep::MarketplaceAdd, CliStep::Install, CliStep::Update] {
            assert!(step.needs_bundled_copy(), "{step:?}");
        }
        for step in [CliStep::MarketplaceRemove, CliStep::Uninstall, CliStep::Enable] {
            assert!(!step.needs_bundled_copy(), "{step:?}");
        }
    }

    #[test]
    fn steps_map_to_claude_plugin_arguments() {
        let dir = Path::new("/h/.agent-loaf/marketplace");
        assert_eq!(
            CliStep::MarketplaceAdd.args(dir),
            ["plugin", "marketplace", "add", "/h/.agent-loaf/marketplace"]
        );
        assert_eq!(
            CliStep::Install.args(dir),
            ["plugin", "install", "agent-loaf@agent-loaf"]
        );
        assert_eq!(CliStep::Update.args(dir), ["plugin", "update", "agent-loaf@agent-loaf"]);
        assert_eq!(
            CliStep::MarketplaceRemove.args(dir),
            ["plugin", "marketplace", "remove", "agent-loaf"]
        );
        assert_eq!(CliStep::Enable.args(dir), ["plugin", "enable", "agent-loaf@agent-loaf"]);
    }

    #[test]
    fn codex_steps_use_codex_commands() {
        use crate::base::agent::Agent;
        let dir = Path::new("/b");
        assert_eq!(
            CliStep::MarketplaceAdd.args_for(Agent::Codex, dir),
            ["plugin", "marketplace", "add", "/b"]
        );
        assert_eq!(
            CliStep::Install.args_for(Agent::Codex, dir),
            ["plugin", "add", "agent-loaf@agent-loaf"]
        );
        // Codex には update がなく、版を上げたマーケットプレイスからもう一度 add すると入れ替わる（2026-10-12 に確認）
        assert_eq!(
            CliStep::Update.args_for(Agent::Codex, dir),
            ["plugin", "add", "agent-loaf@agent-loaf"]
        );
        assert_eq!(
            CliStep::Enable.args_for(Agent::Codex, dir),
            ["plugin", "add", "agent-loaf@agent-loaf"]
        );
        assert_eq!(
            CliStep::Uninstall.args_for(Agent::Codex, dir),
            ["plugin", "remove", "agent-loaf@agent-loaf"]
        );
        assert_eq!(
            CliStep::MarketplaceRemove.args_for(Agent::Codex, dir),
            ["plugin", "marketplace", "remove", "agent-loaf"]
        );
        assert_eq!(
            CliStep::Install.args_for(Agent::ClaudeCode, dir),
            CliStep::Install.args(dir)
        );
    }
}
