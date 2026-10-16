use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::agent::Agent;
use crate::event::{MonitorEvent, TerminalInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    NeedsAction,
    Error,
    Done,
    Running,
    Ended,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetailKind {
    New,
    Permission,
    Question,
    PlanApproval,
    Input,
    Interrupted,
    Subagent,
    Compacting,
    Tool,
    Error,
    Background,
    NotMonitored,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    pub status: Status,
    pub detail_kind: Option<DetailKind>,
    pub detail_text: Option<String>,
}

pub fn transition_for(ev: &MonitorEvent) -> Option<Transition> {
    let detail = &ev.detail;
    let tool_label = match (&detail.tool_name, &detail.summary) {
        (Some(tool), Some(summary)) => Some(format!("{tool}: {summary}")),
        (Some(tool), None) => Some(tool.clone()),
        _ => None,
    };
    let to = |status: Status, kind: Option<DetailKind>, text: Option<String>| {
        Some(Transition {
            status,
            detail_kind: kind,
            detail_text: text,
        })
    };
    let with_text = |kind: DetailKind, text: Option<String>| text.is_some().then_some(kind);
    match ev.event.as_str() {
        "SessionStart" => match detail.source.as_deref() {
            // 圧縮の後にも開始の知らせが届くが、会話は続いているので状態を変えない
            Some("compact") => None,
            Some("resume") => to(Status::Done, None, None),
            _ => to(Status::Done, Some(DetailKind::New), None),
        },
        "UserPromptSubmit" => to(Status::Running, None, None),
        "PreToolUse" | "PermissionRequest" => match detail.tool_name.as_deref() {
            // 質問とプラン承認は、PreToolUse の後に同じツール名の PermissionRequest も届く
            Some("AskUserQuestion") => to(Status::NeedsAction, Some(DetailKind::Question), None),
            Some("ExitPlanMode") => to(Status::NeedsAction, Some(DetailKind::PlanApproval), None),
            _ if ev.event == "PermissionRequest" => to(Status::NeedsAction, Some(DetailKind::Permission), tool_label),
            _ => to(
                Status::Running,
                with_text(DetailKind::Tool, tool_label.clone()),
                tool_label,
            ),
        },
        "Notification" => match detail.notification_type.as_deref() {
            Some("permission_prompt") => to(Status::NeedsAction, Some(DetailKind::Permission), None),
            Some("elicitation_dialog" | "elicitation_url_dialog" | "agent_needs_input") => {
                to(Status::NeedsAction, Some(DetailKind::Input), None)
            }
            Some("idle_prompt") => to(Status::Done, None, None),
            _ => None,
        },
        "Elicitation" => to(Status::NeedsAction, Some(DetailKind::Input), None),
        "PostToolUse" | "PostToolUseFailure" | "ElicitationResult" => to(Status::Running, None, None),
        "SubagentStart" => to(Status::Running, Some(DetailKind::Subagent), None),
        "PreCompact" => to(Status::Running, Some(DetailKind::Compacting), None),
        "Stop" => to(Status::Done, None, None),
        "StopFailure" => to(
            Status::Error,
            with_text(DetailKind::Error, detail.error_type.clone()),
            detail.error_type.clone(),
        ),
        "SessionEnd" => to(Status::Ended, None, None),
        // Codex は Esc での中断を知らせる。Claude Code にはないので、会話の記録から推測している
        "Interrupt" => to(Status::Done, Some(DetailKind::Interrupted), None),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub status: Status,
    pub detail_kind: Option<DetailKind>,
    pub detail_text: Option<String>,
    pub status_since_ms: u64,
    pub first_seen_ms: u64,
    pub last_event_ms: u64,
    pub cwd: String,
    // 移動するときに開くフォルダー。Claude が cd すると cwd は変わるが、エディターのウィンドウは始めたときのフォルダーで開いている
    #[serde(default)]
    pub start_cwd: String,
    pub config_dir: String,
    pub git_branch: Option<String>,
    pub claude_pid: Option<u32>,
    pub terminal: Option<TerminalInfo>,
    pub transcript_path: Option<String>,
    // 数で数えると、開始の知らせのない終了の知らせ（毎ターンのように届く）で、動いているものまで打ち消してしまう
    #[serde(default)]
    pub running_subagents: BTreeSet<String>,
    #[serde(default)]
    pub agent: Agent,
    #[serde(default)]
    pub seen_ms: Option<u64>,
    #[serde(default)]
    pub hidden: bool,
}

impl Session {
    pub fn project(&self) -> String {
        if let Some(name) = Path::new(&self.cwd).file_name() {
            return name.to_string_lossy().into_owned();
        }
        match self.claude_pid {
            Some(pid) => format!("Claude (pid {pid})"),
            None => "Claude".to_string(),
        }
    }
}
