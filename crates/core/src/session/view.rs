use serde::Serialize;

use crate::base::agent::Agent;
use crate::base::messages::{text, Lang, Msg};
use crate::session::state::{DetailKind, Session, Status};
use crate::setup::config_dirs::Monitored;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SessionView {
    pub id: String,
    pub status: Status,
    pub project: String,
    pub detail: Option<String>,
    pub status_since_ms: u64,
    pub branch: Option<String>,
    pub cwd: String,
    pub config_dir: String,
    pub claude_pid: Option<u32>,
    pub agent: Agent,
}

pub fn session_views(
    sessions: &[Session],
    order: &[String],
    monitored: &Monitored,
    lang: Lang,
) -> (Vec<SessionView>, Vec<String>) {
    // 終了したセッションは遅れて届くイベントを捨てるために内部では残すが、画面には出さない。
    // 隠した行も、プロセスが生きている間は状態不明の行を作らないよう、また使われたら戻すよう、内部では残す
    let active: Vec<&Session> = sessions
        .iter()
        .filter(|s| s.status != Status::Ended && !s.hidden && monitored.includes(&s.id, &s.config_dir))
        .collect();
    let (arranged, next_order) = crate::session::order::arrange(&active, order);
    let views = arranged
        .into_iter()
        .map(|session| {
            let shares_project =
                !session.cwd.is_empty() && active.iter().filter(|other| other.cwd == session.cwd).count() > 1;
            SessionView {
                id: session.id.clone(),
                status: session.status,
                project: session.project(),
                detail: detail_label(lang, session.detail_kind, session.detail_text.as_deref()),
                status_since_ms: session.status_since_ms,
                branch: if shares_project {
                    session.git_branch.clone()
                } else {
                    None
                },
                cwd: session.cwd.clone(),
                config_dir: session.config_dir.clone(),
                claude_pid: session.claude_pid,
                agent: session.agent,
            }
        })
        .collect();
    (views, next_order)
}

pub fn status_label(lang: Lang, status: Status) -> String {
    let msg = match status {
        Status::NeedsAction => Msg::StatusNeedsAction,
        Status::Error => Msg::StatusError,
        Status::Done => Msg::StatusDone,
        Status::Running => Msg::StatusRunning,
        Status::Ended => Msg::StatusEnded,
        Status::Unknown => Msg::StatusUnknown,
    };
    text(lang, msg)
}

pub fn detail_label(lang: Lang, kind: Option<DetailKind>, detail_text: Option<&str>) -> Option<String> {
    let head = match kind? {
        DetailKind::Tool | DetailKind::Error => return detail_text.map(String::from),
        DetailKind::NotMonitored => {
            let origin = detail_text
                .map(String::from)
                .unwrap_or_else(|| text(lang, Msg::DetailTerminal));
            return Some(format!("{} - {origin}", text(lang, Msg::DetailNotMonitored)));
        }
        DetailKind::New => Msg::DetailNew,
        DetailKind::Permission => Msg::DetailPermission,
        DetailKind::Question => Msg::DetailQuestion,
        DetailKind::PlanApproval => Msg::DetailPlanApproval,
        DetailKind::Input => Msg::DetailInput,
        DetailKind::Interrupted => Msg::DetailInterrupted,
        DetailKind::Subagent => Msg::DetailSubagent,
        DetailKind::Compacting => Msg::DetailCompacting,
        DetailKind::Background => Msg::DetailBackground,
    };
    let head = text(lang, head);
    Some(match detail_text {
        Some(rest) => format!("{head} · {rest}"),
        None => head,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::store::test_support::ev;
    use crate::session::store::SessionStore;

    fn store_with(events: &[(&str, &str, &str, &str, u32)]) -> SessionStore {
        let mut store = SessionStore::default();
        for (i, (id, cwd, config_dir, branch, pid)) in events.iter().enumerate() {
            let mut e = ev("SessionStart", id, i as u64 + 1);
            e.cwd = cwd.to_string();
            e.config_dir = config_dir.to_string();
            e.git_branch = Some(branch.to_string());
            e.claude_pid = Some(*pid);
            store.apply(&e);
        }
        store
    }

    #[test]
    fn detail_label_joins_kind_and_text_in_each_language() {
        use crate::base::messages::Lang;
        use crate::session::state::DetailKind;
        assert_eq!(
            detail_label(Lang::Ja, Some(DetailKind::Permission), Some("Bash: rm -rf dist")).as_deref(),
            Some("許可待ち · Bash: rm -rf dist")
        );
        assert_eq!(
            detail_label(Lang::En, Some(DetailKind::Permission), Some("Bash: rm -rf dist")).as_deref(),
            Some("Waiting for permission · Bash: rm -rf dist")
        );
        assert_eq!(
            detail_label(Lang::En, Some(DetailKind::Question), None).as_deref(),
            Some("Waiting for your answer")
        );
        assert_eq!(
            detail_label(Lang::En, Some(DetailKind::Tool), Some("Read: a.rs")).as_deref(),
            Some("Read: a.rs"),
            "ツール実行中は中身だけ"
        );
        assert_eq!(
            detail_label(Lang::En, Some(DetailKind::Error), Some("rate_limit")).as_deref(),
            Some("rate_limit"),
            "エラーは種類の名前だけ"
        );
        assert_eq!(
            detail_label(Lang::Ja, Some(DetailKind::NotMonitored), Some("VS Code")).as_deref(),
            Some("監視対象外 - VS Code")
        );
        assert_eq!(
            detail_label(Lang::En, Some(DetailKind::NotMonitored), None).as_deref(),
            Some("Not monitored - Terminal")
        );
        assert_eq!(detail_label(Lang::En, None, None), None);
        assert_eq!(
            status_label(Lang::En, crate::session::state::Status::NeedsAction),
            "Needs action"
        );
    }

    #[test]
    fn hides_sessions_from_dirs_that_are_not_monitored() {
        let store = store_with(&[
            ("a", "/w/api", "/u/.claude", "main", 1),
            ("b", "/w/web", "/u/.claude-work", "main", 2),
        ]);
        let (views, _) = session_views(
            store.sessions(),
            &[],
            &Monitored::new(&["/u/.claude".into()]),
            crate::base::messages::Lang::Ja,
        );
        assert_eq!(views.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(), ["a"]);
    }

    #[test]
    fn keeps_order_and_hides_branch_when_unambiguous() {
        let store = store_with(&[
            ("a", "/w/api", "/u/.claude", "main", 1),
            ("b", "/w/web", "/u/.claude", "dev", 2),
        ]);
        let views = session_views(
            store.sessions(),
            &[],
            &Monitored::new(&["/u/.claude".into()]),
            crate::base::messages::Lang::Ja,
        )
        .0;
        assert_eq!(
            views.iter().map(|v| v.project.as_str()).collect::<Vec<_>>(),
            ["api", "web"]
        );
        assert!(views.iter().all(|v| v.branch.is_none()));
    }

    #[test]
    fn hides_ended_sessions() {
        let mut store = store_with(&[
            ("a", "/w/api", "/u/.claude", "main", 1),
            ("b", "/w/web", "/u/.claude", "main", 2),
        ]);
        let mut end = ev("SessionEnd", "a", 10);
        end.claude_pid = Some(1);
        store.apply(&end);
        let views = session_views(
            store.sessions(),
            &[],
            &Monitored::new(&["/u/.claude".into()]),
            crate::base::messages::Lang::Ja,
        )
        .0;
        assert_eq!(views.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(), ["b"]);
    }

    #[test]
    fn shows_branch_when_same_project_runs_twice() {
        let store = store_with(&[
            ("a", "/w/api", "/u/.claude", "main", 1),
            ("b", "/w/api", "/u/.claude", "fix", 2),
        ]);
        let views = session_views(
            store.sessions(),
            &[],
            &Monitored::new(&["/u/.claude".into()]),
            crate::base::messages::Lang::Ja,
        )
        .0;
        assert_eq!(views[0].branch.as_deref(), Some("main"));
        assert_eq!(views[1].branch.as_deref(), Some("fix"));
    }

    #[test]
    fn composes_every_detail_kind() {
        use crate::base::messages::Lang::Ja;
        use crate::session::state::DetailKind::*;
        let label = |kind, text: Option<&str>| detail_label(Ja, Some(kind), text);
        assert_eq!(detail_label(Ja, None, Some("x")), None);
        assert_eq!(label(Tool, Some("Bash: ls")).as_deref(), Some("Bash: ls"));
        assert_eq!(label(Tool, None), None);
        assert_eq!(label(Error, Some("rate_limit")).as_deref(), Some("rate_limit"));
        assert_eq!(
            label(Permission, Some("Bash: rm")),
            Some(format!("{} · Bash: rm", text(Ja, Msg::DetailPermission)))
        );
        assert_eq!(
            label(NotMonitored, None),
            Some(format!(
                "{} - {}",
                text(Ja, Msg::DetailNotMonitored),
                text(Ja, Msg::DetailTerminal)
            ))
        );
        assert_eq!(
            label(NotMonitored, Some("VS Code")),
            Some(format!("{} - VS Code", text(Ja, Msg::DetailNotMonitored)))
        );
        for (kind, msg) in [
            (New, Msg::DetailNew),
            (Question, Msg::DetailQuestion),
            (PlanApproval, Msg::DetailPlanApproval),
            (Input, Msg::DetailInput),
            (Interrupted, Msg::DetailInterrupted),
            (Subagent, Msg::DetailSubagent),
            (Compacting, Msg::DetailCompacting),
            (Background, Msg::DetailBackground),
        ] {
            assert_eq!(label(kind, None), Some(text(Ja, msg)), "{kind:?}");
        }
    }

    #[test]
    fn views_carry_the_agent() {
        let store = store_with(&[("s1", "/w/api", "/Users/u/.claude", "main", 1)]);
        let (views, _) = session_views(
            store.sessions(),
            &[],
            &Monitored::new(&["/Users/u/.claude".into()]),
            Lang::Ja,
        );
        assert_eq!(views[0].agent, crate::base::agent::Agent::ClaudeCode);
    }

    #[test]
    fn hidden_sessions_are_not_listed() {
        let mut store = store_with(&[
            ("a", "/w/api", "/u/.claude", "main", 1),
            ("b", "/w/api", "/u/.claude", "dev", 2),
        ]);
        for (event, id, pid, ts_ms) in [
            ("UserPromptSubmit", "a", 1, 10),
            ("Stop", "a", 1, 11),
            ("UserPromptSubmit", "b", 2, 12),
        ] {
            let mut e = ev(event, id, ts_ms);
            e.config_dir = "/u/.claude".into();
            e.claude_pid = Some(pid);
            store.apply(&e);
        }
        store.mark_finished_seen(20);
        store.hide_seen_finished(20 + crate::session::store::SEEN_FINISHED_HIDE_MS);
        let (views, _) = session_views(
            store.sessions(),
            &[],
            &Monitored::new(&["/u/.claude".into()]),
            crate::base::messages::Lang::Ja,
        );
        let ids: Vec<&str> = views.iter().map(|v| v.id.as_str()).collect();
        assert_eq!(ids, ["b"]);
        assert_eq!(views[0].branch, None, "隠れた行とはブランチで見分けなくてよい");
    }
}
