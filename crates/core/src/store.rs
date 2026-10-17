use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::agent::Agent;
use crate::event::MonitorEvent;
use crate::notify::{legacy_notification_id, notification_id};
use crate::ps::{is_agent_command, is_interactive_session, launch_origin, ProcRow};
use crate::state::{transition_for, DetailKind, Session, Status, Transition};

pub const UNKNOWN_ID_PREFIX: &str = "pid:";
pub const ENDED_RETENTION_MS: u64 = 5 * 60 * 1000;
pub const INTERRUPT_QUIET_MS: u64 = 15_000;
// pid が一度も分からなかったセッションは生きているか確かめられない。強制終了で終了の知らせが来ないと残り続けるので、長く静かなら終わったとみなす
pub const ORPHAN_TIMEOUT_MS: u64 = 24 * 60 * 60 * 1000;
// 終了の知らせの後もプロセスは後片付けでしばらく残り、/clear では次の開始の知らせまで間がある。その間に状態不明の行を出さない
pub const ENDED_PID_GRACE_MS: u64 = 30_000;
pub const SEEN_FINISHED_HIDE_MS: u64 = 60 * 60 * 1000;

#[derive(Debug, Clone, PartialEq)]
pub struct StatusChange {
    pub session_id: String,
    pub project: String,
    pub from: Option<Status>,
    pub to: Status,
    pub detail_kind: Option<DetailKind>,
    pub detail_text: Option<String>,
    pub ts_ms: u64,
    pub config_dir: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NotificationIds {
    pub id: String,
    pub stale: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SessionStore {
    sessions: Vec<Session>,
}

impl SessionStore {
    pub fn sessions(&self) -> &[Session] {
        &self.sessions
    }

    pub fn apply(&mut self, ev: &MonitorEvent) -> Option<StatusChange> {
        // Codex は 1 つのプロセス（VS Code の app-server）で複数の会話を並べて動かすので、同じ pid でも前の会話を終わりにしない
        if let Some(pid) = ev.claude_pid.filter(|_| ev.agent == Agent::ClaudeCode) {
            self.retire_sessions_sharing_pid(pid, &ev.session_id, ev.ts_ms);
        }
        let (index, is_new) = self.find_or_insert(ev);
        let session = &mut self.sessions[index];
        observe(session, ev);
        let next = transition_for(ev)?;
        if !accepts(session, ev, &next) {
            return None;
        }
        let from = (!is_new).then_some(session.status);
        enter(session, next, ev);
        // 圧縮の開始は transition_for で落ちるので、ここに来る SessionStart は会話の始まりか開き直し
        let started = ev.event == "SessionStart";
        if started {
            // 履歴から開き直した会話は、完了のまま状態が変わらなくても一覧に戻す
            session.hidden = false;
            session.seen_ms = None;
        }
        let change = StatusChange {
            session_id: session.id.clone(),
            project: session.project(),
            from,
            to: session.status,
            detail_kind: session.detail_kind,
            detail_text: session.detail_text.clone(),
            ts_ms: ev.ts_ms,
            config_dir: session.config_dir.clone(),
        };
        // pid 1 は launchd。親が終わって引き取られた会話どうしは、同じウィンドウとは限らない
        if let Some(host_pid) = ev
            .terminal
            .as_ref()
            .and_then(|t| t.host_pid)
            .filter(|&pid| started && pid > 1)
        {
            self.hide_unused_sessions_in_window(host_pid, &ev.session_id);
        }
        Some(change)
    }

    // 同じセッションの通知は最新の 1 つだけを残す。新しい通知の識別子と、通知センターから消す識別子を返す
    pub fn record_notification(&mut self, session_id: &str, now_ms: u64) -> NotificationIds {
        let mut stale = vec![legacy_notification_id(session_id)];
        let session = self.sessions.iter_mut().find(|s| s.id == session_id);
        let previous = session.as_ref().and_then(|s| s.notification_id.clone());
        let mut ts_ms = now_ms;
        while previous.as_deref() == Some(notification_id(session_id, ts_ms).as_str()) {
            ts_ms += 1;
        }
        let id = notification_id(session_id, ts_ms);
        stale.extend(previous);
        if let Some(session) = session {
            session.notification_id = Some(id.clone());
        }
        NotificationIds { id, stale }
    }

    pub fn tracked_pids(&self) -> Vec<u32> {
        let mut pids: Vec<u32> = self.sessions.iter().filter_map(|s| s.claude_pid).collect();
        pids.sort_unstable();
        pids.dedup();
        pids
    }

    pub fn tray_status(&self, shown: impl Fn(&Session) -> bool) -> Option<Status> {
        [Status::NeedsAction, Status::Error, Status::Running]
            .into_iter()
            .find(|&status| self.sessions.iter().any(|s| s.status == status && shown(s)))
    }

    pub fn reconcile(&mut self, rows: &[ProcRow], now_ms: u64, lookup_cwd: impl Fn(u32) -> Option<String>) -> bool {
        // 再起動後などに pid が別のプロセスに再利用されることがあるため、Claude Code と Codex のプロセスだけを生存とみなす
        let alive: HashSet<u32> = rows
            .iter()
            .filter(|r| is_agent_command(&r.args))
            .map(|r| r.pid)
            .collect();
        let before = self.sessions.len();
        self.sessions.retain(|s| {
            !(s.id.starts_with(UNKNOWN_ID_PREFIX) && !s.claude_pid.is_some_and(|pid| alive.contains(&pid)))
        });
        let mut changed = before != self.sessions.len();
        for session in self.sessions.iter_mut() {
            let dead = session.claude_pid.is_some_and(|pid| !alive.contains(&pid));
            let orphaned =
                session.claude_pid.is_none() && now_ms.saturating_sub(session.last_event_ms) >= ORPHAN_TIMEOUT_MS;
            if (dead || orphaned) && session.status != Status::Ended {
                mark_ended(session, now_ms);
                changed = true;
            }
        }
        let known: HashSet<u32> = self
            .sessions
            .iter()
            .filter(|s| s.status != Status::Ended || now_ms.saturating_sub(s.status_since_ms) < ENDED_PID_GRACE_MS)
            .filter_map(|s| s.claude_pid)
            .collect();
        for row in rows {
            if is_interactive_session(&row.args) && !known.contains(&row.pid) {
                let cwd = lookup_cwd(row.pid).unwrap_or_default();
                self.sessions.push(unknown_process_session(row, cwd, now_ms));
                changed = true;
            }
        }
        changed
    }

    // Esc での中断や許可の拒否では Stop が届かないため、静かになったセッションの会話ログを確かめる
    pub fn detect_interrupts(
        &mut self,
        now_ms: u64,
        quiet_ms: u64,
        ends_with_interrupt: impl Fn(&Path) -> bool,
    ) -> bool {
        let mut changed = false;
        for session in self.sessions.iter_mut() {
            let quiet = now_ms.saturating_sub(session.last_event_ms) >= quiet_ms;
            // Codex は中断を Interrupt で知らせ、会話の記録の形も安定した仕様ではないので読まない
            if session.agent == Agent::Codex
                || !matches!(session.status, Status::Running | Status::NeedsAction)
                || !quiet
            {
                continue;
            }
            let Some(transcript) = session.transcript_path.as_deref() else {
                continue;
            };
            if ends_with_interrupt(Path::new(transcript)) {
                session.status = Status::Done;
                // now にすると、判定の直前に書かれて直後に読まれた入力の知らせを古いものとして捨ててしまう
                session.status_since_ms = session.last_event_ms + 1;
                session.detail_kind = Some(DetailKind::Interrupted);
                session.detail_text = None;
                changed = true;
            }
        }
        changed
    }

    // 状態不明の行は保存時点のプロセス情報なので、起動時に捨ててプロセス一覧から作り直す
    pub fn forget_unknown_processes(&mut self) {
        self.sessions.retain(|s| !s.id.starts_with(UNKNOWN_ID_PREFIX));
    }

    pub fn mark_finished_seen(&mut self, now_ms: u64) -> bool {
        let mut changed = false;
        for session in self
            .sessions
            .iter_mut()
            .filter(|s| is_finished(s) && !s.hidden && s.seen_ms.is_none())
        {
            session.seen_ms = Some(now_ms);
            changed = true;
        }
        changed
    }

    // VS Code では新しいチャットを始めても前のチャットのプロセスが残り、終了の知らせも来ないので、見たあと静かな行を隠す
    pub fn hide_seen_finished(&mut self, now_ms: u64) -> bool {
        let mut changed = false;
        for session in self.sessions.iter_mut().filter(|s| is_finished(s) && !s.hidden) {
            let Some(seen_ms) = session.seen_ms else { continue };
            if now_ms.saturating_sub(seen_ms.max(session.last_event_ms)) >= SEEN_FINISHED_HIDE_MS {
                session.hidden = true;
                changed = true;
            }
        }
        changed
    }

    fn hide_unused_sessions_in_window(&mut self, host_pid: u32, session_id: &str) {
        for session in self.sessions.iter_mut() {
            let same_window = session.terminal.as_ref().and_then(|t| t.host_pid) == Some(host_pid);
            if same_window
                && session.id != session_id
                && session.status == Status::Done
                && session.detail_kind == Some(DetailKind::New)
            {
                session.hidden = true;
            }
        }
    }

    pub fn prune(&mut self, now_ms: u64) -> bool {
        let before = self.sessions.len();
        self.sessions
            .retain(|s| !(s.status == Status::Ended && now_ms.saturating_sub(s.status_since_ms) >= ENDED_RETENTION_MS));
        before != self.sessions.len()
    }

    fn find_or_insert(&mut self, ev: &MonitorEvent) -> (usize, bool) {
        if let Some(index) = self.sessions.iter().position(|s| s.id == ev.session_id) {
            return (index, false);
        }
        self.sessions.push(new_session(ev));
        (self.sessions.len() - 1, true)
    }

    // /clear などで同じ Claude プロセスの session_id が変わると、古い行が残り続けるため
    fn retire_sessions_sharing_pid(&mut self, pid: u32, session_id: &str, ts_ms: u64) {
        self.sessions
            .retain(|s| !(s.id.starts_with(UNKNOWN_ID_PREFIX) && s.claude_pid == Some(pid)));
        // 旧セッションのイベントが新セッションの開始後に遅れて届くことがあるため、
        // 終了済みのセッションや、相手より古いイベントでは退役させない
        let sender_retired = self
            .sessions
            .iter()
            .any(|s| s.id == session_id && s.status == Status::Ended);
        if sender_retired {
            return;
        }
        for session in self.sessions.iter_mut() {
            let newer_than_other = ts_ms >= session.last_event_ms;
            if session.claude_pid == Some(pid)
                && session.agent == Agent::ClaudeCode
                && session.id != session_id
                && session.status != Status::Ended
                && newer_than_other
            {
                mark_ended(session, ts_ms);
            }
        }
    }
}

fn new_session(ev: &MonitorEvent) -> Session {
    Session {
        id: ev.session_id.clone(),
        status: Status::Unknown,
        detail_kind: None,
        detail_text: None,
        status_since_ms: ev.ts_ms,
        first_seen_ms: ev.ts_ms,
        last_event_ms: ev.ts_ms,
        cwd: ev.cwd.clone(),
        start_cwd: ev.cwd.clone(),
        config_dir: ev.config_dir.clone(),
        git_branch: None,
        claude_pid: ev.claude_pid,
        terminal: None,
        transcript_path: None,
        running_subagents: Default::default(),
        agent: ev.agent,
        seen_ms: None,
        hidden: false,
        notification_id: None,
    }
}

fn observe(session: &mut Session, ev: &MonitorEvent) {
    if session.start_cwd.is_empty() && !ev.cwd.is_empty() {
        session.start_cwd = ev.cwd.clone();
    }
    // hook は動き始めた時刻を記録してから書くので、読み込みをまたいで古いイベントが後から届くことがある。
    // 最新の値として持つものは、すでに見たイベントより古いもので戻さない
    if ev.ts_ms >= session.last_event_ms {
        session.agent = ev.agent;
        if !ev.cwd.is_empty() {
            session.cwd = ev.cwd.clone();
        }
        session.config_dir = ev.config_dir.clone();
        if ev.claude_pid.is_some() {
            session.claude_pid = ev.claude_pid;
        }
        if ev.git_branch.is_some() {
            session.git_branch = ev.git_branch.clone();
        }
        if ev.terminal.is_some() {
            session.terminal = ev.terminal.clone();
        }
        if ev.transcript_path.is_some() {
            session.transcript_path = ev.transcript_path.clone();
        }
    }
    session.last_event_ms = session.last_event_ms.max(ev.ts_ms);
    // サブエージェントの開始と終了は、届いた順に関係なく数える
    match (ev.event.as_str(), ev.agent_id.as_deref()) {
        ("SubagentStart", Some(agent)) => {
            session.running_subagents.insert(agent.to_string());
        }
        ("SubagentStop", Some(agent)) => {
            session.running_subagents.remove(agent);
        }
        // 会話が始め直されたら前のサブエージェントは残っていない。圧縮では会話が続くので残す
        ("SessionStart", _) if ev.detail.source.as_deref() != Some("compact") => session.running_subagents.clear(),
        _ => {}
    }
}

fn accepts(session: &Session, ev: &MonitorEvent, next: &Transition) -> bool {
    if ev.ts_ms < session.status_since_ms {
        return false;
    }
    if session.status == Status::Ended && ev.event != "SessionStart" {
        return false;
    }
    let from_subagent = ev.agent_id.is_some() && ev.event != "SubagentStart";
    if from_subagent {
        return next.status == Status::NeedsAction
            || (session.status == Status::NeedsAction && next.status == Status::Running);
    }
    if ev.event == "Notification" {
        // 通知は PermissionRequest などの後に届くおまけなので、より詳しい表示を上書きさせない
        if session.status == Status::NeedsAction {
            return false;
        }
        if next.status == Status::Done && session.status == Status::Error {
            return false;
        }
    }
    true
}

fn enter(session: &mut Session, next: Transition, ev: &MonitorEvent) {
    if session.status != next.status {
        session.status_since_ms = ev.ts_ms;
        session.seen_ms = None;
        session.hidden = false;
    }
    session.status = next.status;
    (session.detail_kind, session.detail_text) =
        if next.status == Status::Done && ev.event == "Stop" && !session.running_subagents.is_empty() {
            (Some(DetailKind::Background), None)
        } else {
            (next.detail_kind, next.detail_text)
        };
}

// 裏でサブエージェントが動いている完了は、まだ終わっていない
fn is_finished(session: &Session) -> bool {
    session.status == Status::Done && session.detail_kind != Some(DetailKind::Background)
}

pub(crate) fn mark_ended(session: &mut Session, ts_ms: u64) {
    session.status = Status::Ended;
    session.status_since_ms = ts_ms;
    session.detail_kind = None;
    session.detail_text = None;
}

fn unknown_process_session(row: &ProcRow, cwd: String, now_ms: u64) -> Session {
    let pid = row.pid;
    Session {
        id: format!("{UNKNOWN_ID_PREFIX}{pid}"),
        status: Status::Unknown,
        detail_kind: Some(DetailKind::NotMonitored),
        detail_text: launch_origin(&row.args).editor_name().map(String::from),
        status_since_ms: now_ms,
        first_seen_ms: now_ms,
        last_event_ms: now_ms,
        start_cwd: cwd.clone(),
        cwd,
        config_dir: String::new(),
        git_branch: None,
        claude_pid: Some(pid),
        terminal: None,
        transcript_path: None,
        running_subagents: Default::default(),
        agent: Agent::ClaudeCode,
        seen_ms: None,
        hidden: false,
        notification_id: None,
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use crate::event::{EventDetail, MonitorEvent};

    pub fn ev(event: &str, session: &str, ts_ms: u64) -> MonitorEvent {
        MonitorEvent {
            schema: 1,
            ts_ms,
            event: event.into(),
            session_id: session.into(),
            agent_id: None,
            claude_pid: Some(100),
            config_dir: "/Users/test/.claude".into(),
            cwd: "/work/my-api".into(),
            transcript_path: None,
            detail: EventDetail::default(),
            terminal: None,
            git_branch: None,
            agent: Default::default(),
        }
    }

    pub fn tool(mut event: MonitorEvent, name: &str, summary: Option<&str>) -> MonitorEvent {
        event.detail.tool_name = Some(name.into());
        event.detail.summary = summary.map(str::to_string);
        event
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{ev, tool};
    use super::*;

    #[test]
    fn first_notification_removes_the_one_older_versions_left() {
        let mut store = SessionStore::default();
        store.apply(&ev("UserPromptSubmit", "s1", 1));
        let ids = store.record_notification("s1", 100);
        assert_eq!(ids.id, crate::notify::notification_id("s1", 100));
        assert_eq!(ids.stale, vec![crate::notify::legacy_notification_id("s1")]);
    }

    #[test]
    fn next_notification_removes_the_previous_one_even_after_a_restart() {
        let mut store = SessionStore::default();
        store.apply(&ev("UserPromptSubmit", "s1", 1));
        let first = store.record_notification("s1", 100);
        let restarted: SessionStore = serde_json::from_str(&serde_json::to_string(&store).unwrap()).unwrap();
        let mut store = restarted;
        let second = store.record_notification("s1", 200);
        assert!(second.stale.contains(&first.id));
        assert_ne!(second.id, first.id);
    }

    #[test]
    fn notifications_in_the_same_millisecond_still_get_their_own_ids() {
        let mut store = SessionStore::default();
        store.apply(&ev("UserPromptSubmit", "s1", 1));
        let first = store.record_notification("s1", 100);
        let second = store.record_notification("s1", 100);
        assert_ne!(second.id, first.id);
        assert!(second.stale.contains(&first.id));
    }

    #[test]
    fn other_sessions_keep_their_notifications() {
        let mut store = SessionStore::default();
        store.apply(&ev("UserPromptSubmit", "s1", 1));
        store.apply(&ev("UserPromptSubmit", "s2", 1));
        let first = store.record_notification("s1", 100);
        let other = store.record_notification("s2", 200);
        assert!(!other.stale.contains(&first.id));
    }

    // 種類と中身から作った日本語の文面で比べると、組み立て方もあわせて確かめられる
    fn status_of(store: &SessionStore, id: &str) -> (Status, Option<String>) {
        let s = store.sessions().iter().find(|s| s.id == id).unwrap();
        (
            s.status,
            crate::view::detail_label(crate::messages::Lang::Ja, s.detail_kind, s.detail_text.as_deref()),
        )
    }
    fn start(session: &str, ts_ms: u64, source: &str) -> MonitorEvent {
        let mut e = ev("SessionStart", session, ts_ms);
        e.detail.source = Some(source.into());
        e
    }

    #[test]
    fn compact_restart_keeps_the_current_status() {
        let mut store = SessionStore::default();
        store.apply(&start("s1", 1, "startup"));
        store.apply(&ev("UserPromptSubmit", "s1", 2));
        assert!(store.apply(&start("s1", 3, "compact")).is_none());
        assert_eq!(status_of(&store, "s1"), (Status::Running, None));
    }

    #[test]
    fn resumed_session_is_not_shown_as_new() {
        let mut store = SessionStore::default();
        store.apply(&start("s1", 1, "resume"));
        assert_eq!(status_of(&store, "s1"), (Status::Done, None));
        store.apply(&start("s2", 1, "startup"));
        assert_eq!(status_of(&store, "s2"), (Status::Done, Some("新規".into())));
        store.apply(&start("s3", 1, "clear"));
        assert_eq!(status_of(&store, "s3"), (Status::Done, Some("新規".into())));
    }

    #[test]
    fn start_cwd_stays_at_the_folder_the_session_began_in() {
        let mut store = SessionStore::default();
        let mut first = start("s1", 1, "startup");
        first.cwd = "/w/api".into();
        store.apply(&first);
        let mut later = ev("UserPromptSubmit", "s1", 2);
        later.cwd = "/w/api/sub".into();
        store.apply(&later);
        let session = &store.sessions()[0];
        assert_eq!(session.cwd, "/w/api/sub");
        assert_eq!(session.start_cwd, "/w/api");
    }

    fn notification(session: &str, ts_ms: u64, kind: &str) -> MonitorEvent {
        let mut e = ev("Notification", session, ts_ms);
        e.detail.notification_type = Some(kind.into());
        e
    }

    #[test]
    fn session_start_creates_new_done_session() {
        let mut store = SessionStore::default();
        let change = store.apply(&ev("SessionStart", "s1", 1)).unwrap();
        assert_eq!(change.from, None);
        assert_eq!(change.to, Status::Done);
        assert_eq!(change.project, "my-api");
        assert_eq!(status_of(&store, "s1"), (Status::Done, Some("新規".into())));
    }

    #[test]
    fn prompt_tool_stop_cycle() {
        let mut store = SessionStore::default();
        store.apply(&ev("SessionStart", "s1", 1));
        store.apply(&ev("UserPromptSubmit", "s1", 2));
        store.apply(&tool(ev("PreToolUse", "s1", 3), "Bash", Some("npm test")));
        assert_eq!(
            status_of(&store, "s1"),
            (Status::Running, Some("Bash: npm test".into()))
        );
        let change = store.apply(&ev("Stop", "s1", 4)).unwrap();
        assert_eq!(change.from, Some(Status::Running));
        assert_eq!(status_of(&store, "s1"), (Status::Done, None));
    }

    #[test]
    fn permission_request_then_resume() {
        let mut store = SessionStore::default();
        store.apply(&ev("UserPromptSubmit", "s1", 1));
        store.apply(&tool(ev("PermissionRequest", "s1", 2), "Bash", Some("rm -rf dist")));
        assert_eq!(
            status_of(&store, "s1"),
            (Status::NeedsAction, Some("許可待ち · Bash: rm -rf dist".into()))
        );
        store.apply(&tool(ev("PostToolUse", "s1", 3), "Bash", None));
        assert_eq!(status_of(&store, "s1").0, Status::Running);
    }

    #[test]
    fn permission_notification_keeps_richer_detail() {
        let mut store = SessionStore::default();
        store.apply(&tool(ev("PermissionRequest", "s1", 1), "Bash", Some("rm -rf dist")));
        assert!(store.apply(&notification("s1", 2, "permission_prompt")).is_none());
        assert_eq!(
            status_of(&store, "s1"),
            (Status::NeedsAction, Some("許可待ち · Bash: rm -rf dist".into()))
        );
    }

    #[test]
    fn question_and_plan_approval_need_action() {
        let mut store = SessionStore::default();
        store.apply(&tool(ev("PreToolUse", "s1", 1), "AskUserQuestion", None));
        assert_eq!(status_of(&store, "s1"), (Status::NeedsAction, Some("質問待ち".into())));
        store.apply(&tool(ev("PreToolUse", "s2", 1), "ExitPlanMode", None));
        assert_eq!(status_of(&store, "s2"), (Status::NeedsAction, Some("承認待ち".into())));
    }

    #[test]
    fn question_label_survives_permission_request_and_notification() {
        let mut store = SessionStore::default();
        store.apply(&tool(ev("PreToolUse", "s1", 1), "AskUserQuestion", None));
        store.apply(&tool(ev("PermissionRequest", "s1", 2), "AskUserQuestion", None));
        store.apply(&notification("s1", 3, "permission_prompt"));
        assert_eq!(status_of(&store, "s1"), (Status::NeedsAction, Some("質問待ち".into())));

        let mut plan = SessionStore::default();
        plan.apply(&tool(ev("PreToolUse", "p1", 1), "ExitPlanMode", None));
        plan.apply(&tool(ev("PermissionRequest", "p1", 2), "ExitPlanMode", None));
        plan.apply(&notification("p1", 3, "permission_prompt"));
        assert_eq!(status_of(&plan, "p1"), (Status::NeedsAction, Some("承認待ち".into())));
    }

    #[test]
    fn stop_failure_is_error_with_type() {
        let mut store = SessionStore::default();
        let mut failure = ev("StopFailure", "s1", 1);
        failure.detail.error_type = Some("rate_limit".into());
        store.apply(&failure);
        assert_eq!(status_of(&store, "s1"), (Status::Error, Some("rate_limit".into())));
    }

    #[test]
    fn idle_prompt_does_not_downgrade_needs_action_or_error() {
        let mut store = SessionStore::default();
        store.apply(&tool(ev("PermissionRequest", "s1", 1), "Bash", None));
        assert!(store.apply(&notification("s1", 2, "idle_prompt")).is_none());
        assert_eq!(status_of(&store, "s1").0, Status::NeedsAction);

        let mut failed = SessionStore::default();
        failed.apply(&ev("StopFailure", "s2", 1));
        assert!(failed.apply(&notification("s2", 2, "idle_prompt")).is_none());
        assert_eq!(status_of(&failed, "s2").0, Status::Error);
    }

    #[test]
    fn subagent_tool_use_does_not_change_main_status() {
        let mut store = SessionStore::default();
        store.apply(&ev("UserPromptSubmit", "s1", 1));
        let mut start = ev("SubagentStart", "s1", 2);
        start.agent_id = Some("a1".into());
        store.apply(&start);
        let mut inner = tool(ev("PreToolUse", "s1", 3), "Bash", Some("ls"));
        inner.agent_id = Some("a1".into());
        assert!(store.apply(&inner).is_none());
        assert_eq!(
            status_of(&store, "s1"),
            (Status::Running, Some("サブエージェント実行中".into()))
        );
    }

    #[test]
    fn subagent_permission_request_needs_action() {
        let mut store = SessionStore::default();
        store.apply(&ev("UserPromptSubmit", "s1", 1));
        let mut request = tool(ev("PermissionRequest", "s1", 2), "Bash", Some("ls"));
        request.agent_id = Some("a1".into());
        store.apply(&request);
        assert_eq!(status_of(&store, "s1").0, Status::NeedsAction);
        let mut resumed = tool(ev("PostToolUse", "s1", 3), "Bash", None);
        resumed.agent_id = Some("a1".into());
        store.apply(&resumed);
        assert_eq!(status_of(&store, "s1").0, Status::Running);
    }

    #[test]
    fn stop_with_active_subagent_mentions_background() {
        let mut store = SessionStore::default();
        let mut start = ev("SubagentStart", "s1", 1);
        start.agent_id = Some("a1".into());
        store.apply(&start);
        store.apply(&ev("Stop", "s1", 2));
        assert_eq!(
            status_of(&store, "s1"),
            (Status::Done, Some("バックグラウンド作業あり".into()))
        );
        let mut stop = ev("SubagentStop", "s1", 3);
        stop.agent_id = Some("a1".into());
        store.apply(&stop);
        store.apply(&ev("UserPromptSubmit", "s1", 4));
        store.apply(&ev("Stop", "s1", 5));
        assert_eq!(status_of(&store, "s1"), (Status::Done, None));
    }

    #[test]
    fn subagent_stop_without_start_is_ignored() {
        let mut store = SessionStore::default();
        let mut stop = ev("SubagentStop", "s1", 1);
        stop.agent_id = Some("internal".into());
        store.apply(&stop);
        store.apply(&ev("Stop", "s1", 2));
        assert!(store.sessions()[0].running_subagents.is_empty());
        assert_eq!(status_of(&store, "s1"), (Status::Done, None));
    }

    #[test]
    fn stale_event_does_not_override_newer_status() {
        let mut store = SessionStore::default();
        store.apply(&ev("Stop", "s1", 10));
        assert!(store.apply(&ev("UserPromptSubmit", "s1", 5)).is_none());
        assert_eq!(status_of(&store, "s1").0, Status::Done);
    }

    #[test]
    fn new_session_on_same_process_ends_previous() {
        let mut store = SessionStore::default();
        store.apply(&ev("SessionStart", "old", 1));
        store.apply(&ev("SessionStart", "new", 2));
        assert_eq!(status_of(&store, "old").0, Status::Ended);
        assert_eq!(status_of(&store, "new").0, Status::Done);
    }

    #[test]
    fn events_after_end_are_ignored_except_session_start() {
        let mut store = SessionStore::default();
        store.apply(&ev("SessionEnd", "s1", 1));
        assert!(store.apply(&ev("UserPromptSubmit", "s1", 2)).is_none());
        assert_eq!(status_of(&store, "s1").0, Status::Ended);
        store.apply(&ev("SessionStart", "s1", 3));
        assert_eq!(status_of(&store, "s1").0, Status::Done);
    }

    #[test]
    fn unknown_event_is_ignored_but_session_is_tracked() {
        let mut store = SessionStore::default();
        assert!(store.apply(&ev("SomethingNew", "s1", 1)).is_none());
        assert_eq!(status_of(&store, "s1").0, Status::Unknown);
    }

    #[test]
    fn tray_status_prefers_needs_action_then_error_then_running() {
        let mut store = SessionStore::default();
        let mut a = ev("Stop", "a", 1);
        a.claude_pid = Some(1);
        let mut c = ev("SessionEnd", "c", 1);
        c.claude_pid = Some(3);
        store.apply(&a);
        store.apply(&c);
        assert_eq!(store.tray_status(|_| true), None, "完了と終了だけなら記号なし");
        let mut r = ev("UserPromptSubmit", "r", 1);
        r.claude_pid = Some(5);
        store.apply(&r);
        assert_eq!(store.tray_status(|_| true), Some(Status::Running));
        let mut b = ev("StopFailure", "b", 1);
        b.claude_pid = Some(2);
        store.apply(&b);
        assert_eq!(store.tray_status(|_| true), Some(Status::Error));
        let mut d = tool(ev("PermissionRequest", "d", 1), "Bash", None);
        d.claude_pid = Some(4);
        store.apply(&d);
        assert_eq!(store.tray_status(|_| true), Some(Status::NeedsAction));
    }
    #[test]
    fn tray_status_ignores_sessions_that_are_not_shown() {
        let mut store = SessionStore::default();
        store.apply(&ev("UserPromptSubmit", "hidden", 1));
        assert_eq!(store.tray_status(|_| true), Some(Status::Running));
        assert_eq!(store.tray_status(|s| s.id != "hidden"), None);
    }

    fn proc(pid: u32, args: &str) -> ProcRow {
        ProcRow {
            pid,
            ppid: 1,
            tty: None,
            args: args.into(),
        }
    }

    fn no_cwd(_pid: u32) -> Option<String> {
        None
    }

    #[test]
    fn forgetting_unknown_processes_keeps_real_sessions() {
        let mut store = SessionStore::default();
        store.apply(&ev("Stop", "s1", 1));
        store.reconcile(&[proc(100, "claude"), proc(700, "claude")], 2, no_cwd);
        store.forget_unknown_processes();
        let ids: Vec<_> = store.sessions().iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["s1"]);
    }

    #[test]
    fn unknown_row_shows_project_and_launch_origin() {
        let mut store = SessionStore::default();
        let vscode = "/Users/u/.vscode/extensions/anthropic.claude-code-2.1.289-darwin-arm64/resources/native-binary/claude --output-format stream-json";
        store.reconcile(&[proc(500, vscode)], 10, |pid| {
            (pid == 500).then(|| "/work/api-frontend".to_string())
        });
        let session = &store.sessions()[0];
        assert_eq!(session.project(), "api-frontend");
        assert_eq!(
            (session.detail_kind, session.detail_text.as_deref()),
            (Some(DetailKind::NotMonitored), Some("VS Code"))
        );
    }

    #[test]
    fn dead_process_ends_session() {
        let mut store = SessionStore::default();
        store.apply(&ev("Stop", "s1", 1));
        assert!(store.reconcile(&[proc(999, "claude")], 50, no_cwd));
        assert_eq!(status_of(&store, "s1").0, Status::Ended);
        assert_eq!(
            store.sessions().iter().find(|s| s.id == "s1").unwrap().status_since_ms,
            50
        );
    }

    #[test]
    fn untracked_interactive_claude_becomes_unknown_row() {
        let mut store = SessionStore::default();
        let rows = [
            proc(500, "/Users/u/.local/bin/claude"),
            proc(501, "claude -p hi"),
            proc(502, "/bin/zsh"),
        ];
        assert!(store.reconcile(&rows, 10, no_cwd));
        let ids: Vec<_> = store.sessions().iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["pid:500"]);
        assert_eq!(status_of(&store, "pid:500").0, Status::Unknown);
        assert!(!store.reconcile(&rows, 11, no_cwd), "2 回目は変化なし");
    }

    #[test]
    fn unknown_row_disappears_with_process_or_real_event() {
        let mut store = SessionStore::default();
        store.reconcile(&[proc(100, "claude")], 1, no_cwd);
        store.apply(&ev("SessionStart", "s1", 2));
        let ids: Vec<_> = store.sessions().iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["s1"]);

        let mut other = SessionStore::default();
        other.reconcile(&[proc(700, "claude")], 1, no_cwd);
        assert!(other.reconcile(&[], 2, no_cwd));
        assert!(other.sessions().is_empty());
    }

    #[test]
    fn prune_removes_sessions_ended_long_ago() {
        let mut store = SessionStore::default();
        store.apply(&ev("SessionEnd", "s1", 1_000));
        assert!(!store.prune(1_000 + ENDED_RETENTION_MS - 1));
        assert!(store.prune(1_000 + ENDED_RETENTION_MS));
        assert!(store.sessions().is_empty());
    }

    #[test]
    fn quiet_running_session_with_interrupt_marker_becomes_done() {
        let mut store = SessionStore::default();
        let mut prompt = ev("UserPromptSubmit", "s1", 1_000);
        prompt.transcript_path = Some("/t/s1.jsonl".into());
        store.apply(&prompt);
        assert!(!store.detect_interrupts(1_000 + INTERRUPT_QUIET_MS - 1, INTERRUPT_QUIET_MS, |_| true));
        assert!(!store.detect_interrupts(1_000 + INTERRUPT_QUIET_MS, INTERRUPT_QUIET_MS, |_| false));
        assert!(
            store.detect_interrupts(1_000 + INTERRUPT_QUIET_MS, INTERRUPT_QUIET_MS, |p| p
                == Path::new("/t/s1.jsonl"))
        );
        assert_eq!(status_of(&store, "s1"), (Status::Done, Some("中断".into())));
    }

    #[test]
    fn rejected_permission_becomes_done_when_interrupt_marker_found() {
        let mut store = SessionStore::default();
        let mut request = tool(ev("PermissionRequest", "s1", 1_000), "Bash", Some("ls /var"));
        request.transcript_path = Some("/t/s1.jsonl".into());
        store.apply(&request);
        assert!(store.detect_interrupts(1_000 + INTERRUPT_QUIET_MS, INTERRUPT_QUIET_MS, |_| true));
        assert_eq!(status_of(&store, "s1"), (Status::Done, Some("中断".into())));
    }

    #[test]
    fn done_and_error_sessions_are_not_checked_for_interrupts() {
        let mut store = SessionStore::default();
        let mut stop = ev("Stop", "s1", 1);
        stop.transcript_path = Some("/t/s1.jsonl".into());
        store.apply(&stop);
        assert!(!store.detect_interrupts(1 + INTERRUPT_QUIET_MS, INTERRUPT_QUIET_MS, |_| true));
        assert_eq!(status_of(&store, "s1"), (Status::Done, None));
    }
    #[test]
    fn late_event_from_old_session_does_not_end_newer_session_on_same_process() {
        let mut store = SessionStore::default();
        store.apply(&ev("SessionStart", "a", 1));
        store.apply(&ev("Stop", "a", 2));
        store.apply(&ev("SessionStart", "b", 3));
        let mut late = ev("SubagentStop", "a", 2);
        late.agent_id = Some("internal".into());
        store.apply(&late);
        assert_eq!(status_of(&store, "b").0, Status::Done);
        store.apply(&ev("UserPromptSubmit", "b", 5));
        assert_eq!(status_of(&store, "b").0, Status::Running);
    }

    #[test]
    fn pid_reused_by_unrelated_process_ends_session() {
        let mut store = SessionStore::default();
        store.apply(&ev("Stop", "s1", 1));
        assert!(store.reconcile(&[proc(100, "/usr/bin/some-daemon")], 50, no_cwd));
        assert_eq!(status_of(&store, "s1").0, Status::Ended);
    }

    fn claude_row(pid: u32) -> crate::ps::ProcRow {
        crate::ps::ProcRow {
            pid,
            ppid: 1,
            tty: None,
            args: "/Users/u/.local/bin/claude".into(),
        }
    }

    #[test]
    fn session_without_pid_ends_after_a_day_of_silence() {
        let mut store = SessionStore::default();
        let mut e = ev("UserPromptSubmit", "s1", 1_000);
        e.claude_pid = None;
        store.apply(&e);
        assert!(!store.reconcile(&[], 1_000 + ORPHAN_TIMEOUT_MS - 1, |_| None));
        assert_eq!(store.sessions()[0].status, Status::Running);
        assert!(store.reconcile(&[], 1_000 + ORPHAN_TIMEOUT_MS, |_| None));
        assert_eq!(store.sessions()[0].status, Status::Ended);
    }

    #[test]
    fn idle_session_with_live_pid_stays() {
        let mut store = SessionStore::default();
        store.apply(&ev("Stop", "s1", 1_000));
        store.reconcile(&[claude_row(100)], 1_000 + 7 * ORPHAN_TIMEOUT_MS, |_| None);
        assert_eq!(store.sessions()[0].status, Status::Done);
    }

    #[test]
    fn just_ended_session_does_not_turn_into_an_unknown_row() {
        let mut store = SessionStore::default();
        store.apply(&ev("Stop", "s1", 1_000));
        store.apply(&ev("SessionEnd", "s1", 2_000));
        store.reconcile(&[claude_row(100)], 2_000 + ENDED_PID_GRACE_MS - 1, |_| None);
        assert!(store.sessions().iter().all(|s| !s.id.starts_with(UNKNOWN_ID_PREFIX)));
        store.reconcile(&[claude_row(100)], 2_000 + ENDED_PID_GRACE_MS, |_| None);
        assert!(
            store.sessions().iter().any(|s| s.id == "pid:100"),
            "猶予の後もまだ動いていれば状態不明として出す"
        );
    }

    #[test]
    fn prompt_written_just_before_interrupt_check_is_kept() {
        let mut store = SessionStore::default();
        let mut e = ev("UserPromptSubmit", "s1", 1_000);
        e.transcript_path = Some("/t.jsonl".into());
        store.apply(&e);
        let now = 1_000 + INTERRUPT_QUIET_MS + 500;
        assert!(store.detect_interrupts(now, INTERRUPT_QUIET_MS, |_| true));
        // 判定の直前（now より前）に書かれ、判定の後に読まれた入力の知らせ
        store.apply(&ev("UserPromptSubmit", "s1", now - 100));
        assert_eq!(store.sessions()[0].status, Status::Running);
    }

    fn subagent(event: &str, session: &str, ts_ms: u64, agent: &str) -> MonitorEvent {
        let mut e = ev(event, session, ts_ms);
        e.agent_id = Some(agent.into());
        e
    }

    #[test]
    fn unrelated_subagent_stop_keeps_the_background_label() {
        let mut store = SessionStore::default();
        store.apply(&subagent("SubagentStart", "s1", 1, "a1"));
        store.apply(&subagent("SubagentStop", "s1", 2, "other"));
        store.apply(&ev("Stop", "s1", 3));
        assert_eq!(
            status_of(&store, "s1"),
            (Status::Done, Some("バックグラウンド作業あり".into()))
        );
    }

    #[test]
    fn new_session_start_forgets_subagents_whose_stop_was_missed() {
        let mut store = SessionStore::default();
        store.apply(&subagent("SubagentStart", "s1", 1, "a1"));
        store.apply(&start("s1", 2, "clear"));
        store.apply(&ev("Stop", "s1", 3));
        assert_eq!(status_of(&store, "s1"), (Status::Done, None));
    }

    #[test]
    fn compact_keeps_running_subagents() {
        let mut store = SessionStore::default();
        store.apply(&subagent("SubagentStart", "s1", 1, "a1"));
        store.apply(&start("s1", 2, "compact"));
        assert_eq!(store.sessions()[0].running_subagents.len(), 1);
    }

    #[test]
    fn partial_check_keeps_known_and_unknown_rows() {
        let mut store = SessionStore::default();
        store.apply(&ev("Stop", "s1", 1_000));
        store.reconcile(&[claude_row(100), claude_row(200)], 2_000, |_| None);
        assert!(store.sessions().iter().any(|s| s.id == "pid:200"));
        let pids = store.tracked_pids();
        assert_eq!(pids, vec![100, 200]);
        // 一覧を閉じている間は、知っている pid の行だけを渡す
        store.reconcile(&[claude_row(100), claude_row(200)], 5_000, |_| None);
        assert_eq!(status_of(&store, "s1").0, Status::Done);
        assert!(store.sessions().iter().any(|s| s.id == "pid:200"));
        store.reconcile(&[claude_row(100)], 8_000, |_| None);
        assert!(
            store.sessions().iter().all(|s| s.id != "pid:200"),
            "終わった状態不明の行は消える"
        );
    }

    fn codex(event: &str, session: &str, ts_ms: u64) -> MonitorEvent {
        let mut e = ev(event, session, ts_ms);
        e.agent = crate::agent::Agent::Codex;
        e.claude_pid = Some(500);
        e
    }

    fn codex_row(pid: u32) -> crate::ps::ProcRow {
        crate::ps::ProcRow { pid, ppid: 1, tty: None, args: "/Users/u/.vscode/extensions/openai.chatgpt-26.1007.21434-darwin-arm64/bin/macos-aarch64/codex -c features.code_mode_host=true app-server".into() }
    }

    #[test]
    fn codex_conversations_in_one_process_live_side_by_side() {
        let mut store = SessionStore::default();
        store.apply(&codex("SessionStart", "a", 1));
        store.apply(&codex("UserPromptSubmit", "a", 2));
        store.apply(&codex("SessionStart", "b", 3));
        store.apply(&codex("UserPromptSubmit", "b", 4));
        assert_eq!(
            status_of(&store, "a").0,
            Status::Running,
            "同じ app-server の別の会話で終了にしない"
        );
        assert_eq!(status_of(&store, "b").0, Status::Running);
        store.reconcile(&[codex_row(500)], 5, |_| None);
        assert_eq!(
            status_of(&store, "a").0,
            Status::Running,
            "codex のプロセスは生きているとみなす"
        );
        assert!(
            store.sessions().iter().all(|s| !s.id.starts_with(UNKNOWN_ID_PREFIX)),
            "codex を状態不明の行にしない"
        );
        store.apply(&codex("SessionEnd", "a", 6));
        assert_eq!(status_of(&store, "a").0, Status::Ended);
        store.reconcile(&[], 7, |_| None);
        assert_eq!(status_of(&store, "b").0, Status::Ended, "プロセスがなくなれば終わり");
    }

    #[test]
    fn claude_sessions_sharing_a_pid_still_retire() {
        let mut store = SessionStore::default();
        store.apply(&ev("SessionStart", "old", 1));
        store.apply(&ev("SessionStart", "new", 2));
        assert_eq!(status_of(&store, "old").0, Status::Ended, "/clear のときの今の動き");
    }

    #[test]
    fn codex_interrupt_is_done_interrupted_and_transcripts_are_not_read() {
        let mut store = SessionStore::default();
        let mut start = codex("UserPromptSubmit", "a", 1_000);
        start.transcript_path = Some("/t.jsonl".into());
        store.apply(&start);
        assert!(
            !store.detect_interrupts(1_000 + INTERRUPT_QUIET_MS, INTERRUPT_QUIET_MS, |_| true),
            "Codex の記録は読まない"
        );
        store.apply(&codex("Interrupt", "a", 2_000));
        assert_eq!(status_of(&store, "a"), (Status::Done, Some("中断".into())));
        assert_eq!(store.sessions()[0].agent, crate::agent::Agent::Codex);
    }

    const SEEN: u64 = 10_000;

    fn finished(store: &mut SessionStore, session: &str) {
        store.apply(&start(session, 1, "startup"));
        store.apply(&ev("UserPromptSubmit", session, 2));
        store.apply(&ev("Stop", session, 3));
    }

    fn hidden(store: &SessionStore, id: &str) -> bool {
        store.sessions().iter().find(|s| s.id == id).unwrap().hidden
    }

    #[test]
    fn finished_session_hides_an_hour_after_it_was_seen() {
        let mut store = SessionStore::default();
        finished(&mut store, "s1");
        assert!(store.mark_finished_seen(SEEN));
        assert!(!store.mark_finished_seen(SEEN + 5), "一度見たら、見た時刻は最初のまま");
        assert!(!store.hide_seen_finished(SEEN + SEEN_FINISHED_HIDE_MS - 1));
        assert!(!hidden(&store, "s1"));
        assert!(store.hide_seen_finished(SEEN + SEEN_FINISHED_HIDE_MS));
        assert!(hidden(&store, "s1"));
    }

    #[test]
    fn finished_session_nobody_has_seen_stays() {
        let mut store = SessionStore::default();
        finished(&mut store, "s1");
        store.hide_seen_finished(SEEN + 100 * SEEN_FINISHED_HIDE_MS);
        assert!(!hidden(&store, "s1"), "通知を見逃しても、一覧を開けば残っている");
    }

    #[test]
    fn interrupted_session_also_hides_after_it_was_seen() {
        let mut store = SessionStore::default();
        store.apply(&codex("UserPromptSubmit", "a", 1));
        store.apply(&codex("Interrupt", "a", 2));
        store.mark_finished_seen(SEEN);
        store.hide_seen_finished(SEEN + SEEN_FINISHED_HIDE_MS);
        assert!(hidden(&store, "a"));
    }

    #[test]
    fn sessions_still_working_or_waiting_are_never_marked_seen() {
        let mut store = SessionStore::default();
        store.apply(&ev("UserPromptSubmit", "running", 1));
        store.apply(&ev("UserPromptSubmit", "waiting", 1));
        store.apply(&notification("waiting", 2, "permission_prompt"));
        store.apply(&ev("UserPromptSubmit", "background", 1));
        store.apply(&subagent("SubagentStart", "background", 2, "x"));
        store.apply(&ev("Stop", "background", 3));
        assert_eq!(status_of(&store, "background").0, Status::Done);
        assert!(
            !store.mark_finished_seen(SEEN),
            "裏でサブエージェントが動いている完了も、まだ終わっていない"
        );
        store.hide_seen_finished(SEEN + SEEN_FINISHED_HIDE_MS);
        assert!(store.sessions().iter().all(|s| !s.hidden));
    }

    #[test]
    fn finishing_again_after_being_seen_needs_to_be_seen_again() {
        let mut store = SessionStore::default();
        finished(&mut store, "s1");
        store.mark_finished_seen(SEEN);
        store.apply(&ev("UserPromptSubmit", "s1", SEEN + 1));
        store.apply(&ev("Stop", "s1", SEEN + 2));
        store.hide_seen_finished(SEEN + SEEN_FINISHED_HIDE_MS + 10);
        assert!(!hidden(&store, "s1"), "新しい完了はまだ見ていない");
    }

    #[test]
    fn hidden_session_comes_back_when_it_is_used_again() {
        let mut store = SessionStore::default();
        finished(&mut store, "s1");
        store.mark_finished_seen(SEEN);
        store.hide_seen_finished(SEEN + SEEN_FINISHED_HIDE_MS);
        let later = SEEN + 3 * SEEN_FINISHED_HIDE_MS;
        let change = store.apply(&ev("UserPromptSubmit", "s1", later)).unwrap();
        assert_eq!(change.to, Status::Running);
        assert!(!hidden(&store, "s1"));
    }

    fn start_in(session: &str, ts_ms: u64, pid: u32, host_pid: u32) -> MonitorEvent {
        let mut e = start(session, ts_ms, "startup");
        e.claude_pid = Some(pid);
        e.terminal = Some(crate::event::TerminalInfo {
            host_pid: Some(host_pid),
            ..Default::default()
        });
        e
    }

    #[test]
    fn new_conversation_in_the_same_window_hides_conversations_never_used() {
        let mut store = SessionStore::default();
        store.apply(&start_in("unused", 1, 101, 40));
        store.apply(&start_in("used", 2, 102, 40));
        store.apply(&ev("UserPromptSubmit", "used", 3));
        store.apply(&ev("Stop", "used", 4));
        store.apply(&start_in("other-window", 5, 103, 41));
        store.apply(&start_in("latest", 6, 104, 40));
        assert!(hidden(&store, "unused"), "何も送っていない新規の行は、見逃すものがない");
        assert!(!hidden(&store, "used"), "完了は見るまで残す");
        assert!(!hidden(&store, "other-window"));
        assert!(!hidden(&store, "latest"));
    }

    #[test]
    fn an_older_event_arriving_late_does_not_roll_back_the_folder() {
        // hook は動き始めた時刻を記録してから書くので、1 秒ごとの読み込みをまたぐと、古いイベントが後から届く
        let mut store = SessionStore::default();
        let mut newer = ev("PreToolUse", "s1", 10);
        newer.cwd = "/w/web".into();
        store.apply(&newer);
        let mut older = ev("PostToolUse", "s1", 9);
        older.cwd = "/w/api".into();
        store.apply(&older);
        assert_eq!(store.sessions()[0].cwd, "/w/web");
        assert_eq!(store.sessions()[0].project(), "web");
    }

    #[test]
    fn reopening_a_hidden_conversation_brings_it_back() {
        let mut store = SessionStore::default();
        finished(&mut store, "s1");
        store.mark_finished_seen(SEEN);
        store.hide_seen_finished(SEEN + SEEN_FINISHED_HIDE_MS);
        assert!(hidden(&store, "s1"));
        store.apply(&start("s1", SEEN + 2 * SEEN_FINISHED_HIDE_MS, "resume"));
        assert!(
            !hidden(&store, "s1"),
            "履歴から開き直した会話は、何も送らなくても一覧に出る"
        );
        let seen = store.sessions().iter().find(|s| s.id == "s1").unwrap().seen_ms;
        assert_eq!(seen, None, "開き直したら、また見るまで隠さない");
    }

    #[test]
    fn compacting_another_conversation_does_not_hide_new_rows() {
        let mut store = SessionStore::default();
        store.apply(&start_in("unused", 1, 101, 40));
        let mut compact = start_in("busy", 2, 102, 40);
        compact.detail.source = Some("compact".into());
        store.apply(&compact);
        assert!(!hidden(&store, "unused"), "圧縮は新しい会話の始まりではない");
    }

    #[test]
    fn conversations_adopted_by_launchd_are_not_one_window() {
        let mut store = SessionStore::default();
        store.apply(&start_in("unused", 1, 101, 1));
        store.apply(&start_in("latest", 2, 102, 1));
        assert!(
            !hidden(&store, "unused"),
            "親が終わって launchd に引き取られた会話どうしは、同じウィンドウとは限らない"
        );
    }
}
