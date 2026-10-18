export type Agent = "claude_code" | "codex";

export type Status = "needs_action" | "error" | "done" | "running" | "ended" | "unknown";

export interface SessionView {
  id: string;
  status: Status;
  project: string;
  detail: string | null;
  status_since_ms: number;
  branch: string | null;
  cwd: string;
  config_dir: string;
  claude_pid: number | null;
  agent: Agent;
}

export interface Preferences {
  notify_on: Status[];
  launch_at_login: boolean;
}

export type DirState = "installed" | "not_installed" | "missing" | "checking" | "needs_trust";

export interface ConfigDirView {
  path: string;
  display: string;
  state: DirState;
  busy: boolean;
  agent: Agent;
}

export interface SetupView {
  dirs: ConfigDirView[];
  claude_found: boolean;
  codex_found: boolean;
  notifications_allowed: boolean;
  needs_attention: boolean;
  open_settings_on_start: boolean;
}

export type UpdateState =
  | { kind: "idle" }
  | { kind: "available"; version: string }
  | { kind: "downloading"; version: string; percent: number }
  | { kind: "failed"; version: string };
