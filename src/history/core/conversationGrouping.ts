import type { Conversation } from "../../chat/core/chatTypes";

export interface ConversationGroup {
  key: string;
  label: string;
  conversations: Conversation[];
}

export const GROUP_LABELS: Record<string, string> = {
  pinned: "common.time.pinned",
  today: "common.time.today",
  yesterday: "common.time.yesterday",
  week: "common.time.last_7_days",
  month: "common.time.last_30_days",
  older: "common.time.earlier",
};

export function groupConversations(
  convs: Conversation[],
  translateFn: (key: string) => string,
): ConversationGroup[] {
  const groups: ConversationGroup[] = [];

  const pinned = convs.filter((c) => c.is_pinned);
  if (pinned.length > 0)
    groups.push({
      key: "pinned",
      label: translateFn(GROUP_LABELS.pinned),
      conversations: pinned,
    });

  const unpinned = convs.filter((c) => !c.is_pinned);

  const now = new Date();
  const todayStart = new Date(
    now.getFullYear(),
    now.getMonth(),
    now.getDate(),
  ).getTime();
  const yesterdayStart = todayStart - 86400000;
  const weekStart = todayStart - 6 * 86400000;
  const monthStart = todayStart - 29 * 86400000;

  const today = unpinned.filter((c) => c.updated_at >= todayStart);
  const yesterday = unpinned.filter(
    (c) => c.updated_at >= yesterdayStart && c.updated_at < todayStart,
  );
  const week = unpinned.filter(
    (c) => c.updated_at >= weekStart && c.updated_at < yesterdayStart,
  );
  const month = unpinned.filter(
    (c) => c.updated_at >= monthStart && c.updated_at < weekStart,
  );
  const older = unpinned.filter((c) => c.updated_at < monthStart);

  if (today.length)
    groups.push({
      key: "today",
      label: translateFn(GROUP_LABELS.today),
      conversations: today,
    });
  if (yesterday.length)
    groups.push({
      key: "yesterday",
      label: translateFn(GROUP_LABELS.yesterday),
      conversations: yesterday,
    });
  if (week.length)
    groups.push({
      key: "week",
      label: translateFn(GROUP_LABELS.week),
      conversations: week,
    });
  if (month.length)
    groups.push({
      key: "month",
      label: translateFn(GROUP_LABELS.month),
      conversations: month,
    });
  if (older.length)
    groups.push({
      key: "older",
      label: translateFn(GROUP_LABELS.older),
      conversations: older,
    });

  return groups;
}
