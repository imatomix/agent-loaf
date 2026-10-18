import { t } from "../../lib/i18n";

export function formatElapsed(ms: number): string {
  const seconds = Math.max(0, Math.floor(ms / 1000));
  if (seconds < 60) return t("elapsed.seconds", { n: seconds });
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return t("elapsed.minutes", { n: minutes });
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return t("elapsed.hours", { n: hours });
  return t("elapsed.days", { n: Math.floor(hours / 24) });
}
