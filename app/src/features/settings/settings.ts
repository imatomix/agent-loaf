import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ICON_DELETE, ICON_REPLAY, ICON_SETTINGS } from "./icons";
import { t } from "../../lib/i18n";
import type { ConfigDirView, Preferences, SetupView, Status, UpdateState } from "../../lib/types";

// 実行中は頻繁に変わり通知が多すぎるので選ばせない。以前の設定で選ばれていた場合は、保存し直しても消さずに残す
const NOTIFIABLE: Status[] = ["needs_action", "error", "done"];
const optionsEl = document.getElementById("notify-options")!;
const launchEl = document.getElementById("launch-at-login") as HTMLInputElement;
const errorEl = document.getElementById("settings-error")!;
let current: Preferences | null = null;

function showError(message: string | null): void {
  errorEl.hidden = message === null;
  errorEl.textContent = message ?? "";
}

let saving: Promise<void> = Promise.resolve();

// 描いたときの値から作ると、保存が終わる前に続けて押した変更が古い値で上書きされる。押した時点の最新の値から作り、順に保存する
function update(change: (prefs: Preferences) => Preferences): void {
  if (!current) return;
  const next = change(current);
  current = next;
  render();
  saving = saving.then(() => persist(next));
}

async function persist(next: Preferences): Promise<void> {
  try {
    await invoke("save_settings", { settings: next });
    showError(null);
  } catch (error) {
    showError(String(error));
    // 保存できなかったので、画面を実際に保存されている値に戻す
    current = await invoke<Preferences>("get_settings");
    render();
  }
}

function render(): void {
  if (!current) return;
  const settings = current;
  optionsEl.replaceChildren(
    ...NOTIFIABLE.map((status) => {
      const label = document.createElement("label");
      label.className = "menu-item";
      const input = document.createElement("input");
      input.type = "checkbox";
      input.className = "checkbox";
      input.checked = settings.notify_on.includes(status);
      input.addEventListener("change", () => {
        update((prefs) => ({
          ...prefs,
          notify_on: input.checked ? [...prefs.notify_on, status] : prefs.notify_on.filter((s) => s !== status),
        }));
      });
      label.append(input, t(`status.${status}`));
      return label;
    }),
  );
  launchEl.checked = settings.launch_at_login;
}

export function setupSettingsMenu(): void {
  launchEl.addEventListener("change", () => update((prefs) => ({ ...prefs, launch_at_login: launchEl.checked })));
  document.getElementById("quit")!.addEventListener("click", () => void invoke("quit_app"));
  void invoke<Preferences>("get_settings").then((settings) => {
    current = settings;
    render();
  });
}

const dirsEl = document.getElementById("config-dirs")!;
const dirsEmptyEl = document.getElementById("config-dirs-empty")!;
const notifyOffEl = document.getElementById("notify-off")!;
const badgeEl = document.getElementById("settings-badge")!;
const claudeForm = document.getElementById("claude-path-form") as HTMLFormElement;
const claudeInput = document.getElementById("claude-path") as HTMLInputElement;
const codexForm = document.getElementById("codex-path-form") as HTMLFormElement;
const codexInput = document.getElementById("codex-path") as HTMLInputElement;
const setupErrorEl = document.getElementById("setup-error")!;

// アイコンは固定の文字列なので innerHTML で入れる。パスなど任意の文字列は textContent で入れること
function iconButton(icon: string, label: string, disabled: boolean, onClick: () => void): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "icon-button";
  button.title = label;
  button.setAttribute("aria-label", label);
  button.innerHTML = icon;
  button.disabled = disabled;
  button.addEventListener("click", onClick);
  return button;
}

function listItem(child: HTMLElement): HTMLElement {
  const item = document.createElement("li");
  item.append(child);
  return item;
}

function renderDir(dir: ConfigDirView): HTMLElement[] {
  const item = document.createElement("li");
  item.className = "menu-item config-dir";
  const name = document.createElement("span");
  name.className = "grow path";
  name.textContent = dir.display;
  name.title = dir.path;
  const actions = document.createElement("span");
  actions.className = "actions";
  if (dir.state === "not_installed") {
    actions.append(iconButton(ICON_REPLAY, t("settings.retry"), dir.busy, () => void invoke("retry_config_dir", { path: dir.path })));
  }
  const checking = dir.state === "checking";
  actions.append(iconButton(ICON_DELETE, t("settings.remove"), dir.busy || checking, () => void invoke("remove_config_dir", { path: dir.path })));
  item.append(name, actions);
  if (dir.state !== "needs_trust") return [item];
  // Codex の hook は利用者が許可するまで動かない。許可の画面は VS Code の中にないので、ターミナルで開く
  const note = document.createElement("p");
  note.className = "menu-note";
  note.textContent = t("settings.codex_trust_needed");
  const open = document.createElement("button");
  open.type = "button";
  open.className = "menu-item centered";
  open.textContent = t("settings.codex_trust_open");
  open.disabled = dir.busy;
  open.addEventListener("click", () => {
    page.keepPageOnFocus();
    invoke("open_codex_trust", { path: dir.path })
      .then(() => {
        setupErrorEl.hidden = true;
      })
      .catch((error) => {
        setupErrorEl.hidden = false;
        setupErrorEl.textContent = String(error);
      });
  });
  return [item, listItem(note), listItem(open)];
}

interface PageControl {
  keepPageOnFocus(): void;
  openSettings(): void;
}

let page: PageControl;

function renderSetup(setup: SetupView): void {
  badgeEl.hidden = !setup.needs_attention;
  notifyOffEl.hidden = setup.notifications_allowed;
  dirsEmptyEl.hidden = setup.dirs.length > 0;
  dirsEl.replaceChildren(...setup.dirs.flatMap(renderDir));
  claudeForm.hidden = setup.claude_found;
  codexForm.hidden = setup.codex_found;
  if (setup.open_settings_on_start) page.openSettings();
}

export function setupClaudeSection(control: PageControl): void {
  page = control;
  const openSettings = document.getElementById("open-notification-settings")!;
  openSettings.innerHTML = ICON_SETTINGS;
  openSettings.addEventListener("click", () => void invoke("open_notification_settings"));
  document.getElementById("add-config-dir")!.addEventListener("click", () => {
    page.keepPageOnFocus();
    void invoke("add_config_dir");
  });
  claudeForm.addEventListener("submit", (event) => {
    event.preventDefault();
    invoke("set_claude_path", { path: claudeInput.value })
      .then(() => {
        setupErrorEl.hidden = true;
      })
      .catch((error) => {
        setupErrorEl.hidden = false;
        setupErrorEl.textContent = String(error);
      });
  });
  codexForm.addEventListener("submit", (event) => {
    event.preventDefault();
    invoke("set_codex_path", { path: codexInput.value })
      .then(() => {
        setupErrorEl.hidden = true;
      })
      .catch((error) => {
        setupErrorEl.hidden = false;
        setupErrorEl.textContent = String(error);
      });
  });
  void listen<SetupView>("setup-updated", (event) => renderSetup(event.payload));
  void invoke<SetupView>("get_setup").then(renderSetup);
}

const updateSectionEl = document.getElementById("update-section")!;
const updateNoteEl = document.getElementById("update-note")!;
const updateActionEl = document.getElementById("update-action") as HTMLButtonElement;
const versionSectionEl = document.getElementById("version-section")!;
const appVersionEl = document.getElementById("app-version")!;
let currentVersion = "";

function renderUpdate(update: UpdateState): void {
  updateSectionEl.hidden = update.kind === "idle";
  versionSectionEl.hidden = update.kind !== "idle";
  updateActionEl.hidden = update.kind === "downloading" || update.kind === "idle";
  updateActionEl.classList.toggle("retry", update.kind === "failed");
  switch (update.kind) {
    case "idle":
      return;
    case "available":
      updateNoteEl.textContent = t("update.available", { new: update.version, current: currentVersion });
      updateActionEl.textContent = t("update.install");
      return;
    case "downloading":
      updateNoteEl.textContent = t("update.downloading", { percent: update.percent });
      return;
    case "failed":
      updateNoteEl.textContent = t("update.failed", { current: currentVersion });
      // アイコンは固定の文字列なので innerHTML、文言は textContent で入れる
      updateActionEl.innerHTML = ICON_REPLAY;
      updateActionEl.append(t("settings.retry"));
      return;
  }
}

export function setupUpdateSection(): void {
  updateActionEl.addEventListener("click", () => void invoke("install_update"));
  void listen<UpdateState>("update-updated", (event) => renderUpdate(event.payload));
  // 一覧は開くたびに作り直すので、開いたときの状態を受け取る
  void Promise.all([getVersion(), invoke<boolean>("is_release_build")]).then(([version, release]) => {
    currentVersion = version;
    appVersionEl.textContent = release ? version : `${version} dev`;
    return invoke<UpdateState>("get_update").then(renderUpdate);
  });
}
