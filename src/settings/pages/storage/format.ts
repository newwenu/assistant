import { translateKey } from "@/i18n";
import { formatBytes } from "@/utils/formatBytes";

export type TagTone = "neutral" | "success" | "warning" | "error" | "info";

export function tagColor(tone: TagTone): string {
  switch (tone) {
    case "success":
      return "success";
    case "warning":
      return "warning";
    case "error":
      return "error";
    case "info":
      return "processing";
    default:
      return "default";
  }
}

export function formatTimestamp(ts: number): string {
  if (!ts) return "";
  const d = new Date(ts * 1000);
  return d.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function timeText(ts: number | null): string {
  return ts === null
    ? translateKey("settings.storage.common.time_unknown")
    : formatTimestamp(ts);
}

export function bytesOrDash(bytes: number | null): string {
  return bytes === null
    ? translateKey("settings.storage.common.bytes_unknown")
    : formatBytes(bytes);
}

export function countText(count: number | null): string | null {
  if (count === null || count <= 0) return null;
  return translateKey("settings.storage.common.count_value", { count });
}
