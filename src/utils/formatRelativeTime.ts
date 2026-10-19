import { translateKey } from "../i18n";

export function formatRelativeTime(timestamp: number): string {
  const diff = Date.now() - timestamp;
  if (diff < 60000) return translateKey("common.time.just_now");
  if (diff < 3600000)
    return translateKey("common.time.minutes_ago", {
      count: Math.floor(diff / 60000),
    });
  if (diff < 86400000)
    return translateKey("common.time.hours_ago", {
      count: Math.floor(diff / 3600000),
    });
  if (diff < 172800000) return translateKey("common.time.yesterday");
  return new Date(timestamp).toLocaleDateString();
}
