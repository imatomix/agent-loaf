use crate::base::messages::{self, Lang, Msg};
use crate::session::state::{DetailKind, Status};
use crate::session::store::StatusChange;
use crate::session::view::{detail_label, status_label};

#[derive(Debug, Clone, PartialEq)]
pub struct Notice {
    pub session_id: String,
    pub title: String,
    pub body: String,
}

const NOTIFICATION_ID_PREFIX: &str = "session:";

// macOS は同じ識別子の通知が通知センターに残っていると、バナーも音も出さずに中身だけ差し替えることがある。
// 毎回別の識別子にして、同じセッションの古い通知は送る側で消す
pub fn notification_id(session_id: &str, ts_ms: u64) -> String {
    format!("{NOTIFICATION_ID_PREFIX}{session_id}:{ts_ms}")
}

// 0.5.1 までの識別子。通知センターに残っている古い通知を消すのに使う
pub fn legacy_notification_id(session_id: &str) -> String {
    format!("{NOTIFICATION_ID_PREFIX}{session_id}")
}

pub fn session_of_notification(id: &str) -> Option<&str> {
    let rest = id.strip_prefix(NOTIFICATION_ID_PREFIX)?;
    // 0.5.1 までは時刻を付けていなかった。セッション ID は UUID なので ':' を含まない
    Some(rest.rsplit_once(':').map_or(rest, |(session_id, _)| session_id))
}

pub fn notice_for(change: &StatusChange, notify_on: &[Status], lang: Lang) -> Option<Notice> {
    // 現れたばかりのセッション（from が None）は、何かが終わったり止まったりしたわけではない
    if change.from.is_none() || change.from == Some(change.to) || !notify_on.contains(&change.to) {
        return None;
    }
    let (headline, body) = match (change.to, change.detail_kind) {
        // 要対応は「何を待っているか」が見出しで、ツールの中身は本文に回す
        (Status::NeedsAction, Some(kind)) if kind != DetailKind::Tool => (
            detail_label(lang, Some(kind), None).unwrap_or_default(),
            change.detail_text.clone().unwrap_or_default(),
        ),
        (status, kind) => (
            status_label(lang, status),
            detail_label(lang, kind, change.detail_text.as_deref()).unwrap_or_default(),
        ),
    };
    let title = messages::format(
        lang,
        Msg::NoticeTitle,
        &[("project", &change.project), ("headline", &headline)],
    );
    Some(Notice {
        session_id: change.session_id.clone(),
        title,
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::messages::Lang;
    use crate::session::state::DetailKind;

    fn change(from: Option<Status>, to: Status, kind: Option<DetailKind>, text: Option<&str>) -> StatusChange {
        StatusChange {
            config_dir: String::new(),
            session_id: "s1".into(),
            project: "my-api".into(),
            from,
            to,
            detail_kind: kind,
            detail_text: text.map(str::to_string),
            ts_ms: 1,
        }
    }

    const DEFAULT: [Status; 2] = [Status::NeedsAction, Status::Error];

    #[test]
    fn permission_notice_splits_title_and_body() {
        let c = change(
            Some(Status::Running),
            Status::NeedsAction,
            Some(DetailKind::Permission),
            Some("Bash: rm -rf dist"),
        );
        assert_eq!(
            notice_for(&c, &DEFAULT, Lang::Ja).unwrap(),
            Notice {
                session_id: "s1".into(),
                title: "my-api — 許可待ち".into(),
                body: "Bash: rm -rf dist".into()
            }
        );
        assert_eq!(
            notice_for(&c, &DEFAULT, Lang::En).unwrap(),
            Notice {
                session_id: "s1".into(),
                title: "my-api — Waiting for permission".into(),
                body: "Bash: rm -rf dist".into()
            }
        );
    }

    #[test]
    fn question_notice_without_body() {
        let c = change(
            Some(Status::Running),
            Status::NeedsAction,
            Some(DetailKind::Question),
            None,
        );
        assert_eq!(
            notice_for(&c, &DEFAULT, Lang::Ja).unwrap(),
            Notice {
                session_id: "s1".into(),
                title: "my-api — 質問待ち".into(),
                body: String::new()
            }
        );
    }

    #[test]
    fn error_notice_uses_status_label() {
        let c = change(
            Some(Status::Running),
            Status::Error,
            Some(DetailKind::Error),
            Some("rate_limit"),
        );
        assert_eq!(
            notice_for(&c, &DEFAULT, Lang::En).unwrap(),
            Notice {
                session_id: "s1".into(),
                title: "my-api — Error".into(),
                body: "rate_limit".into()
            }
        );
        assert_eq!(
            notice_for(&c, &DEFAULT, Lang::Ja).unwrap(),
            Notice {
                session_id: "s1".into(),
                title: "my-api — エラー停止".into(),
                body: "rate_limit".into()
            }
        );
    }

    #[test]
    fn each_notification_gets_its_own_id() {
        assert_ne!(notification_id("s1", 1_000), notification_id("s1", 2_000));
    }

    #[test]
    fn notification_id_leads_back_to_its_session() {
        assert_eq!(session_of_notification(&notification_id("s1", 1_000)), Some("s1"));
        assert_eq!(
            session_of_notification(&notification_id("019a-codex-thread", 5)),
            Some("019a-codex-thread")
        );
    }

    #[test]
    fn ids_from_older_versions_still_lead_to_their_session() {
        assert_eq!(
            session_of_notification("session:cc4312e7-1ba0-4a13-8984-e7a065963c99"),
            Some("cc4312e7-1ba0-4a13-8984-e7a065963c99")
        );
    }

    #[test]
    fn older_versions_used_one_id_per_session() {
        assert_eq!(legacy_notification_id("s1"), "session:s1");
        assert_eq!(session_of_notification(&legacy_notification_id("s1")), Some("s1"));
    }

    #[test]
    fn ids_from_other_sources_belong_to_no_session() {
        assert_eq!(session_of_notification("something-else"), None);
    }

    #[test]
    fn no_notice_when_a_session_just_appears() {
        assert!(notice_for(
            &change(None, Status::Done, Some(DetailKind::New), None),
            &[Status::Done],
            Lang::Ja
        )
        .is_none());
    }

    #[test]
    fn no_notice_when_status_unchanged_or_not_selected() {
        assert!(notice_for(
            &change(
                Some(Status::NeedsAction),
                Status::NeedsAction,
                Some(DetailKind::Permission),
                None
            ),
            &DEFAULT,
            Lang::Ja
        )
        .is_none());
        assert!(notice_for(
            &change(Some(Status::Running), Status::Done, None, None),
            &DEFAULT,
            Lang::Ja
        )
        .is_none());
        assert!(notice_for(
            &change(Some(Status::Running), Status::Done, None, None),
            &[Status::Done],
            Lang::Ja
        )
        .is_some());
    }
}
