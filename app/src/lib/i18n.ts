import { invoke } from "@tauri-apps/api/core";

let messages: Record<string, string> = {};

export async function loadMessages(): Promise<void> {
  try {
    messages = await invoke<Record<string, string>>("get_messages");
  } catch (error) {
    // 対応表がなくてもキーの文字列で表示して動き続ける。一覧が開かないよりはよい
    console.error("failed to load messages", error);
  }
  document.documentElement.lang = messages["meta.lang"] ?? "en";
}

export function t(key: string, args: Record<string, string | number> = {}): string {
  const template = messages[key] ?? key;
  // 1 回で置き換え、入れた値（パスなど）の中の {…} は置き換えない
  return template.replace(/\{(\w+)\}/g, (whole, name: string) => (name in args ? String(args[name]) : whole));
}

// HTML に書いた固定の文言を差し替える。文言は textContent で入れ、HTML として解釈させない
export function applyStaticTexts(root: ParentNode): void {
  for (const node of root.querySelectorAll<HTMLElement>("[data-i18n]")) node.textContent = t(node.dataset.i18n!);
  for (const node of root.querySelectorAll<HTMLElement>("[data-i18n-title]")) {
    const label = t(node.dataset.i18nTitle!);
    node.title = label;
    node.setAttribute("aria-label", label);
  }
}
