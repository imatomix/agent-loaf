use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Lang {
    Ja,
    En,
}

impl Lang {
    pub fn from_tag(tag: &str) -> Lang {
        if tag.to_ascii_lowercase().starts_with("ja") {
            Lang::Ja
        } else {
            Lang::En
        }
    }
}

macro_rules! messages {
    ($($variant:ident => $key:literal),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Msg { $($variant),* }

        impl Msg {
            pub const ALL: &'static [Msg] = &[$(Msg::$variant),*];

            pub fn key(&self) -> &'static str {
                match self { $(Msg::$variant => $key),* }
            }
        }
    };
}

// Rust から引く文言だけをここに並べる。画面だけで使う文言は表にあればよく、ここには書かない
messages! {
    StatusNeedsAction => "status.needs_action",
    StatusError => "status.error",
    StatusDone => "status.done",
    StatusRunning => "status.running",
    StatusEnded => "status.ended",
    StatusUnknown => "status.unknown",
    DetailNew => "detail.new",
    DetailPermission => "detail.permission",
    DetailQuestion => "detail.question",
    DetailPlanApproval => "detail.plan_approval",
    DetailInput => "detail.input",
    DetailInterrupted => "detail.interrupted",
    DetailSubagent => "detail.subagent",
    DetailCompacting => "detail.compacting",
    DetailBackground => "detail.background",
    DetailNotMonitored => "detail.not_monitored",
    DetailTerminal => "detail.terminal",
    DialogChooseConfigDir => "dialog.choose_config_dir",
    NoticeTitle => "notice.title",
    ErrorInstallFailed => "error.install_failed",
    ErrorRemoveFailed => "error.remove_failed",
    InfoRemovedWithoutClaude => "info.removed_without_claude",
    InfoRemovedWithoutCodex => "info.removed_without_codex",
    ErrorCodexNotFound => "error.codex_not_found",
    ErrorCodexPathInvalid => "error.codex_path_invalid",
    ErrorCodexTrustFailed => "error.codex_trust_failed",
    ErrorNotMonitoredCodexDir => "error.not_monitored_codex_dir",
    ErrorMarketplaceUnavailable => "error.marketplace_unavailable",
    ErrorClaudeNotFound => "error.claude_not_found",
    ErrorClaudePathInvalid => "error.claude_path_invalid",
    ErrorTimeout => "error.timeout",
    ErrorUnreadableStatus => "error.unreadable_status",
    ErrorCliSpawn => "error.cli_spawn",
    ErrorCliWait => "error.cli_wait",
    ErrorUnknownReason => "error.unknown_reason",
    ErrorSaveSettings => "error.save_settings",
    ErrorSaveOrder => "error.save_order",
    ErrorAutostart => "error.autostart",
}

pub fn table(lang: Lang) -> &'static HashMap<String, String> {
    static JA: OnceLock<HashMap<String, String>> = OnceLock::new();
    static EN: OnceLock<HashMap<String, String>> = OnceLock::new();
    let (cell, source) = match lang {
        Lang::Ja => (&JA, include_str!("../../locales/ja.json")),
        Lang::En => (&EN, include_str!("../../locales/en.json")),
    };
    cell.get_or_init(|| serde_json::from_str(source).expect("embedded message table is unreadable"))
}

pub fn table_by_key(lang: Lang, key: &str) -> Option<&'static str> {
    table(lang).get(key).map(String::as_str)
}

pub fn text(lang: Lang, msg: Msg) -> String {
    table_by_key(lang, msg.key()).unwrap_or(msg.key()).to_string()
}

pub fn format(lang: Lang, msg: Msg, args: &[(&str, &str)]) -> String {
    fill(&text(lang, msg), args)
}

// 順に replace すると、先に入れた値（パスやエラーの文）の中の {reason} なども置き換えてしまうので、文言を 1 回だけ読む
fn fill(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let value = after.find('}').and_then(|close| {
            let name = &after[..close];
            args.iter().find(|(n, _)| *n == name).map(|(_, value)| (*value, close))
        });
        match value {
            Some((value, close)) => {
                out.push_str(value);
                rest = &after[close + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_each_placeholder_once() {
        assert_eq!(
            fill("{dir}: {reason}", &[("dir", "/x/{reason}"), ("reason", "boom")]),
            "/x/{reason}: boom"
        );
        assert_eq!(fill("{missing} {dir}", &[("dir", "d")]), "{missing} d");
        assert_eq!(fill("open { brace", &[]), "open { brace");
    }
    use std::collections::BTreeSet;

    fn placeholders(text: &str) -> BTreeSet<String> {
        let mut found = BTreeSet::new();
        let mut rest = text;
        while let Some(start) = rest.find('{') {
            let Some(end) = rest[start..].find('}') else { break };
            found.insert(rest[start + 1..start + end].to_string());
            rest = &rest[start + end + 1..];
        }
        found
    }

    #[test]
    fn both_languages_have_the_same_keys() {
        let ja: BTreeSet<&String> = table(Lang::Ja).keys().collect();
        let en: BTreeSet<&String> = table(Lang::En).keys().collect();
        assert_eq!(ja, en);
    }

    #[test]
    fn every_msg_exists_in_both_languages() {
        for msg in Msg::ALL {
            for lang in [Lang::Ja, Lang::En] {
                assert!(table(lang).contains_key(msg.key()), "{} が {lang:?} にない", msg.key());
            }
        }
    }

    #[test]
    fn placeholders_match_across_languages() {
        for (key, ja) in table(Lang::Ja) {
            assert_eq!(placeholders(ja), placeholders(&table(Lang::En)[key]), "{key}");
        }
    }

    #[test]
    fn formats_values_into_the_text() {
        assert_eq!(text(Lang::En, Msg::StatusDone), "Done");
        assert_eq!(text(Lang::Ja, Msg::StatusDone), "完了");
        assert_eq!(
            format(Lang::En, Msg::NoticeTitle, &[("project", "api"), ("headline", "Done")]),
            "api — Done"
        );
        assert_eq!(
            format(Lang::Ja, Msg::ErrorTimeout, &[]),
            "時間切れ（応答がありませんでした）"
        );
    }

    #[test]
    fn language_tags_starting_with_ja_are_japanese() {
        assert_eq!(Lang::from_tag("ja"), Lang::Ja);
        assert_eq!(Lang::from_tag("ja-JP"), Lang::Ja);
        assert_eq!(Lang::from_tag("ja_JP.UTF-8"), Lang::Ja);
        assert_eq!(Lang::from_tag("en"), Lang::En);
        assert_eq!(Lang::from_tag("fr-FR"), Lang::En);
        assert_eq!(Lang::from_tag(""), Lang::En);
    }
}
