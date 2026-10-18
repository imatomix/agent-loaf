use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::setup::claude_cli::CliError;

pub const PLUGIN_ID: &str = "agent-loaf@agent-loaf";
pub const MARKETPLACE_NAME: &str = "agent-loaf";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginPresence {
    Absent,
    Enabled { version: String },
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketplacePresence {
    Absent,
    Bundled,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inspection {
    pub plugin: PluginPresence,
    pub marketplace: MarketplacePresence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DirState {
    Installed,
    NotInstalled,
    Missing,
    Checking,
    // Codex のプラグインは入っているが、hook がまだ許可されていない
    NeedsTrust,
}

impl Inspection {
    pub fn state(&self) -> DirState {
        match self.plugin {
            PluginPresence::Enabled { .. } => DirState::Installed,
            PluginPresence::Absent | PluginPresence::Disabled => DirState::NotInstalled,
        }
    }
}

#[derive(Deserialize)]
struct PluginEntry {
    id: String,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    version: String,
}

#[derive(Deserialize)]
struct MarketplaceEntry {
    name: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    path: Option<String>,
}

pub fn parse_inspection(
    plugin_list_json: &str,
    marketplace_list_json: &str,
    bundled_dir: &Path,
) -> Result<Inspection, CliError> {
    let plugins: Vec<PluginEntry> = serde_json::from_str(plugin_list_json).map_err(|_| CliError::Unreadable)?;
    let marketplaces: Vec<MarketplaceEntry> =
        serde_json::from_str(marketplace_list_json).map_err(|_| CliError::Unreadable)?;
    let plugin = match plugins.into_iter().find(|p| p.id == PLUGIN_ID) {
        None => PluginPresence::Absent,
        Some(p) if p.enabled => PluginPresence::Enabled { version: p.version },
        Some(_) => PluginPresence::Disabled,
    };
    let bundled = normalized(&bundled_dir.to_string_lossy());
    let marketplace = match marketplaces.into_iter().find(|m| m.name == MARKETPLACE_NAME) {
        None => MarketplacePresence::Absent,
        Some(m) if m.source == "directory" && m.path.as_deref().map(normalized) == Some(bundled.clone()) => {
            MarketplacePresence::Bundled
        }
        Some(_) => MarketplacePresence::Other,
    };
    Ok(Inspection { plugin, marketplace })
}

fn normalized(path: &str) -> String {
    path.trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUNDLED: &str = "/Users/tester/.agent-loaf/marketplace";

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("{}/tests/fixtures/cli/{name}.json", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    fn inspect(plugins: &str, marketplaces: &str) -> Inspection {
        parse_inspection(&fixture(plugins), &fixture(marketplaces), Path::new(BUNDLED)).unwrap()
    }

    #[test]
    fn nothing_installed() {
        let i = inspect("plugin-list-empty", "marketplace-list-empty");
        assert_eq!(
            i,
            Inspection {
                plugin: PluginPresence::Absent,
                marketplace: MarketplacePresence::Absent
            }
        );
        assert_eq!(i.state(), DirState::NotInstalled);
    }

    #[test]
    fn installed_from_bundled_marketplace() {
        let i = inspect("plugin-list-bundled", "marketplace-list-bundled");
        assert_eq!(
            i.plugin,
            PluginPresence::Enabled {
                version: "0.1.0".into()
            }
        );
        assert_eq!(i.marketplace, MarketplacePresence::Bundled);
        assert_eq!(i.state(), DirState::Installed);
    }

    #[test]
    fn installed_from_elsewhere_counts_as_installed() {
        let i = inspect("plugin-list-other", "marketplace-list-other");
        assert_eq!(i.marketplace, MarketplacePresence::Other);
        assert_eq!(i.state(), DirState::Installed);
    }

    #[test]
    fn disabled_counts_as_not_installed() {
        let i = inspect("plugin-list-disabled", "marketplace-list-bundled");
        assert_eq!(i.plugin, PluginPresence::Disabled);
        assert_eq!(i.state(), DirState::NotInstalled);
    }

    #[test]
    fn trailing_slash_on_bundled_path_still_matches() {
        let i = parse_inspection(
            "[]",
            &fixture("marketplace-list-bundled"),
            Path::new(&format!("{BUNDLED}/")),
        )
        .unwrap();
        assert_eq!(i.marketplace, MarketplacePresence::Bundled);
    }

    #[test]
    fn broken_or_unexpected_output_is_an_error_not_a_panic() {
        for (plugins, marketplaces) in [("", "[]"), ("[]", "{oops"), ("{\"a\":1}", "[]"), ("[]", "null")] {
            assert_eq!(
                parse_inspection(plugins, marketplaces, Path::new(BUNDLED)).unwrap_err(),
                crate::setup::claude_cli::CliError::Unreadable
            );
        }
    }

    #[test]
    fn unknown_fields_and_other_plugins_are_ignored() {
        let plugins = r#"[{"id":"x@y","enabled":true,"version":"9"},{"id":"agent-loaf@agent-loaf","enabled":true,"version":"0.2.0","new":1}]"#;
        let i = parse_inspection(plugins, "[]", Path::new(BUNDLED)).unwrap();
        assert_eq!(
            i.plugin,
            PluginPresence::Enabled {
                version: "0.2.0".into()
            }
        );
    }
}
