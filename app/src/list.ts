import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { setupDrag } from "./drag";
import { formatElapsed } from "./format";
import { setupClaudeSection, setupSettingsMenu, setupUpdateSection } from "./settings";
import { applyStaticTexts, loadMessages, t } from "./i18n";
import claudeCodeIcon from "./assets/claude-code.svg";
import codexIcon from "./assets/codex.svg";
import type { Agent, SessionView, Status } from "./types";

type Page = "agents" | "unknown" | "menu";

const WINDOW_WIDTH = 300;
const panelEl = document.getElementById("panel")!;
const listEl = document.getElementById("sessions")!;
const menuEl = document.getElementById("menu")!;
const pageButtons = document.querySelectorAll<HTMLButtonElement>("[data-page]");
const expanded = new Set<string>();
let sessions: SessionView[] = [];
let page: Page = "agents";
let dragger: ReturnType<typeof setupDrag>;

// 作業中の長さと、あなたを待たせている長さだけに意味がある。完了や状態不明の時刻は判断の役に立たない
const ELAPSED_STATUSES = new Set<Status>(["running", "needs_action", "error"]);

// コマンドやパスなど任意の文字列を表示するため、textContent だけで組み立てて HTML として解釈させない
function el<K extends keyof HTMLElementTagNameMap>(tag: K, className: string, text?: string): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function metaText(session: SessionView): string {
  const parts = [session.detail ?? t(`status.${session.status}`)];
  if (session.branch) parts.push(session.branch);
  return parts.join(" - ");
}

const AGENT_ICONS: Record<Agent, { src: string; name: string }> = {
  claude_code: { src: claudeCodeIcon, name: "Claude Code" },
  codex: { src: codexIcon, name: "Codex" },
};

function applyAgentIcon(icon: HTMLImageElement, agent: Agent): void {
  const { src, name } = AGENT_ICONS[agent];
  if (icon.getAttribute("src") === src) return;
  icon.src = src;
  // 名前は製品名なので訳さない
  icon.alt = name;
  icon.title = name;
}

function profileName(configDir: string): string {
  return configDir.split("/").filter(Boolean).pop() ?? "";
}

function renderDetails(session: SessionView): HTMLElement {
  const details = el("div", "details");
  details.append(el("span", "", t("details.location", { path: session.cwd || t("details.unknown_value") })));
  if (session.status === "unknown") {
    details.append(
      el("span", "", t("details.not_monitored_help")),
    );
  } else {
    details.append(el("span", "", t("details.profile", { name: profileName(session.config_dir) || t("details.unknown_value") })));
  }
  return details;
}

// Figma の keyboard_arrow_down（9×5）。開いた行では CSS で上下を逆さにする
const ICON_CHEVRON = `<svg width="9" height="5.55" viewBox="240 -641 480 296" fill="currentColor" aria-hidden="true"><path d="M480-345 240-585l56-56 184 184 184-184 56 56-240 240Z"/></svg>`;

function toggleDetails(id: string): void {
  if (expanded.has(id)) expanded.delete(id);
  else expanded.add(id);
  render();
}

interface RowView {
  row: HTMLLIElement;
  dot: HTMLSpanElement;
  project: HTMLSpanElement;
  elapsed: HTMLSpanElement;
  icon: HTMLImageElement;
  meta: HTMLSpanElement;
  toggle: HTMLButtonElement | null;
  text: HTMLDivElement;
  details: HTMLElement | null;
  jumpable: boolean;
}

// 描くたびに行を作り直すと、押している間に行が入れ替わってクリックが届かず、詳細で選んだ文字も消える。
// 会話ごとに行を残し、変わった中身だけを書き換える
const rowViews = new Map<string, RowView>();
let emptyEl: HTMLLIElement | null = null;

function setText(node: HTMLElement, text: string): void {
  if (node.textContent !== text) node.textContent = text;
}

function createRow(session: SessionView): RowView {
  const id = session.id;
  // 状態不明の行は移動しない（行を押すと詳細が開く）ので、詳細のボタンは要らない
  const jumpable = session.status !== "unknown";
  const row = el("li", "row");
  if (jumpable) row.dataset.id = id;
  const dotBox = el("span", "dot-box");
  const dot = el("span", "dot");
  dotBox.append(dot);
  const project = el("span", "project");
  const elapsed = el("span", "elapsed");
  const headline = el("div", "line");
  headline.append(project, elapsed);
  const icon = el("img", "agent-icon");
  const meta = el("span", "meta");
  const metaWithIcon = el("span", "meta-with-icon");
  metaWithIcon.append(icon, meta);
  const subline = el("div", "line");
  subline.append(metaWithIcon);
  let toggle: HTMLButtonElement | null = null;
  if (jumpable) {
    toggle = el("button", "details-toggle");
    toggle.type = "button";
    toggle.innerHTML = ICON_CHEVRON;
    toggle.addEventListener("click", (event) => {
      event.stopPropagation();
      toggleDetails(id);
    });
    subline.append(toggle);
  }
  const text = el("div", "text");
  text.append(headline, subline);
  row.append(dotBox, text);
  row.addEventListener("click", (event) => {
    if (dragger.consumeClick()) return;
    // 詳細の場所はコピーできるようにしているので、文字を選んだときは移動しない
    if ((event.target as HTMLElement).closest(".details") || getSelection()?.toString()) return;
    if (!jumpable) {
      toggleDetails(id);
      return;
    }
    // 移動できなかったときは、アプリが一覧を開き直して詳細を出す（show-details）
    void invoke("jump_session", { id });
  });
  return { row, dot, project, elapsed, icon, meta, toggle, text, details: null, jumpable };
}

function updateRow(view: RowView, session: SessionView, now: number): void {
  const dotClass = `dot dot-${session.status}`;
  if (view.dot.className !== dotClass) view.dot.className = dotClass;
  setText(view.project, session.project);
  setText(view.elapsed, ELAPSED_STATUSES.has(session.status) ? formatElapsed(now - session.status_since_ms) : "");
  applyAgentIcon(view.icon, session.agent);
  setText(view.meta, metaText(session));
  const open = expanded.has(session.id);
  if (view.toggle) {
    view.toggle.classList.toggle("open", open);
    const label = t(open ? "details.hide" : "details.show");
    view.toggle.title = label;
    view.toggle.setAttribute("aria-label", label);
  }
  if (!open) {
    view.details?.remove();
    view.details = null;
    return;
  }
  const details = renderDetails(session);
  if (!view.details) {
    view.text.append(details);
    view.details = details;
  } else if (view.details.textContent !== details.textContent) {
    view.details.replaceWith(details);
    view.details = details;
  }
}

function renderSessions(): void {
  const now = Date.now();
  const unknownPage = page === "unknown";
  const shown = sessions.filter((session) => (session.status === "unknown") === unknownPage);
  if (shown.length === 0) {
    for (const view of rowViews.values()) view.row.remove();
    rowViews.clear();
    if (!emptyEl) {
      emptyEl = el("li", "empty");
      emptyEl.append(el("span", "", t("list.empty")), el("span", "empty-note", t("list.empty_note")));
    }
    if (!emptyEl.isConnected) listEl.prepend(emptyEl);
    return;
  }
  emptyEl?.remove();
  const keep = new Set<string>();
  shown.forEach((session, index) => {
    let view = rowViews.get(session.id);
    if (view && view.jumpable !== (session.status !== "unknown")) {
      view.row.remove();
      view = undefined;
    }
    if (!view) {
      view = createRow(session);
      rowViews.set(session.id, view);
    }
    updateRow(view, session, now);
    keep.add(session.id);
    // 並びが変わったときだけ動かす。同じ行を動かすので、押している行が消えることはない
    const current = listEl.children[index];
    if (current !== view.row) listEl.insertBefore(view.row, current ?? null);
  });
  for (const [id, view] of rowViews) {
    if (keep.has(id)) continue;
    view.row.remove();
    rowViews.delete(id);
  }
}

// 一覧が見えている間だけ描く。閉じている間も 1 秒ごとに描き直すと、常駐しているだけで WebKit が CPU を使い続ける
let listVisible = false;
let ticker: number | undefined;

function render(): void {
  if (listVisible) draw();
}

function draw(): void {
  if (dragger.isDragging()) return;
  const hasUnknown = sessions.some((session) => session.status === "unknown");
  if (page === "unknown" && !hasUnknown) page = "agents";
  for (const button of pageButtons) {
    button.classList.toggle("selected", button.dataset.page === page);
    if (button.dataset.page === "unknown") button.hidden = !hasUnknown;
  }
  listEl.hidden = page === "menu";
  menuEl.hidden = page !== "menu";
  if (page !== "menu") renderSessions();
}

function showPage(next: Page): void {
  // プラグインの状態を読み直すと claude を設定ディレクトリごとに起動するので、一覧を開くたびではなく設定画面を開いたときだけにする
  if (next === "menu" && page !== "menu") void invoke("refresh_setup");
  page = next;
  render();
}

for (const button of pageButtons) {
  button.addEventListener("click", () => showPage(button.dataset.page as Page));
}

// 透明な部分もクリックを受け止めてしまうので、ウィンドウを角丸の板の大きさにぴったり合わせる
new ResizeObserver(() => {
  const height = Math.ceil(panelEl.getBoundingClientRect().height);
  void getCurrentWindow().setSize(new LogicalSize(WINDOW_WIDTH, height));
}).observe(panelEl);

// 設定ページのまま見せたい場面（~/.claude が見つからない起動、ダイアログやアラートを閉じた後）では、次の 1 回のフォーカスで一覧に戻さない
let keepPageOnNextFocus = false;

function keepPageOnFocus(): void {
  keepPageOnNextFocus = true;
}


dragger = setupDrag(listEl, (id, beforeId) => {
  const moving = sessions.find((session) => session.id === id);
  if (moving) {
    const rest = sessions.filter((session) => session.id !== id);
    const index = beforeId === null ? rest.length : rest.findIndex((session) => session.id === beforeId);
    rest.splice(index < 0 ? rest.length : index, 0, moving);
    sessions = rest;
  }
  render();
  void invoke("move_session", { id, beforeId });
});

async function openPendingDetails(): Promise<void> {
  const id = await invoke<string | null>("take_pending_details");
  if (!id) return;
  showPage("agents");
  if (!expanded.has(id)) toggleDetails(id);
}

// 対応表を受け取る前に描くと、キーの文字列が一瞬見えてしまう
async function start(): Promise<void> {
  await loadMessages();
  applyStaticTexts(document);
  // フォーカスで一覧を描くので、対応表を受け取ってから登録する
  void listen("keep-page", keepPageOnFocus);
  // 移動できなかったときは、そのセッションの詳細を開いて見せる
  void listen("show-details", () => void openPendingDetails());
  void getCurrentWindow().onFocusChanged(({ payload: focused }) => {
    listVisible = focused;
    if (!focused) {
      window.clearInterval(ticker);
      ticker = undefined;
      // 閉じても画面は捨てずに隠すので、隠している間に最初の表示へ戻しておく。開いた瞬間に前の画面が見えないようにするため
      if (!keepPageOnNextFocus) {
        page = "agents";
        expanded.clear();
        draw();
      }
      return;
    }
    ticker ??= window.setInterval(render, 1000);
    if (keepPageOnNextFocus) keepPageOnNextFocus = false;
    else page = "agents";
    render();
    void invoke("refresh_notification_access");
  });
  setupSettingsMenu();
  setupClaudeSection({
    keepPageOnFocus,
    openSettings: () => {
      keepPageOnFocus();
      showPage("menu");
    },
  });
  setupUpdateSection();

  void listen<SessionView[]>("sessions-updated", (event) => {
    sessions = event.payload;
    const ids = new Set(sessions.map((session) => session.id));
    for (const id of expanded) if (!ids.has(id)) expanded.delete(id);
    render();
  });
  void invoke<SessionView[]>("get_sessions").then((initial) => {
    sessions = initial;
    render();
    // 読み込みの前に送られた show-details は受け取れない（起動した直後など）ので、預けてあるものを受け取る
    void openPendingDetails();
  });
  // 画面の読み込みが、最初のフォーカスの知らせより先に終わるとは限らない
  if (await getCurrentWindow().isFocused()) {
    listVisible = true;
    ticker ??= window.setInterval(render, 1000);
    render();
  }
}

void start();
