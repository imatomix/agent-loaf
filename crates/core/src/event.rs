use serde::{Deserialize, Serialize};

use crate::agent::Agent;

pub const SCHEMA_VERSION: u32 = 1;
pub const SUMMARY_MAX_CHARS: usize = 120;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonitorEvent {
    pub schema: u32,
    pub ts_ms: u64,
    pub event: String,
    pub session_id: String,
    pub agent_id: Option<String>,
    pub claude_pid: Option<u32>,
    pub config_dir: String,
    #[serde(default)]
    pub cwd: String,
    pub transcript_path: Option<String>,
    #[serde(default)]
    pub detail: EventDetail,
    pub terminal: Option<TerminalInfo>,
    pub git_branch: Option<String>,
    #[serde(default)]
    pub agent: Agent,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EventDetail {
    pub tool_name: Option<String>,
    pub summary: Option<String>,
    pub notification_type: Option<String>,
    pub error_type: Option<String>,
    // SessionStart の種類（startup・resume・clear・compact）
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TerminalInfo {
    pub term_program: Option<String>,
    pub iterm_session_id: Option<String>,
    pub tmux: Option<String>,
    pub tmux_pane: Option<String>,
    pub entrypoint: Option<String>,
    pub tty: Option<String>,
    // 会話のプロセスの親。VS Code では拡張のプロセスで、同じウィンドウの会話を見分けるのに使う
    #[serde(default)]
    pub host_pid: Option<u32>,
}
