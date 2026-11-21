import type { I18nPath } from "@/i18n";

export type EntryColumnKey = "name" | "size" | "policy" | "cleanup";

export const ENTRY_COLUMN_ORDER: readonly EntryColumnKey[] = [
  "name",
  "size",
  "policy",
  "cleanup",
];

export const ENTRY_COLUMN_WIDTHS = {
  name: "minmax(160px, 1.6fr)",
  size: "minmax(90px, 0.7fr)",
  policy: "minmax(180px, 2.2fr)",
  cleanup: "minmax(88px, 0.6fr)",
} as const satisfies Record<EntryColumnKey, string>;

export const ENTRY_GRID_TEMPLATE = ENTRY_COLUMN_ORDER.map(
  (key) => ENTRY_COLUMN_WIDTHS[key],
).join(" ");

export const ENTRY_ROW_GAP_PX = 16;

export const ENTRY_ROW_PADDING_X_PX = 12;

export const ENTRY_COLUMN_LABEL_KEYS = {
  name: "settings.storage.entries.columns.name",
  size: "settings.storage.entries.columns.size",
  policy: "settings.storage.entries.columns.policy",
  cleanup: "settings.storage.entries.columns.cleanup",
} as const satisfies Record<EntryColumnKey, I18nPath>;

export function groupOfKey(key: string): string {
  const slash = key.indexOf("/");
  return slash === -1 ? key : key.slice(0, slash);
}

export const COLLAPSIBLE_GROUPS = ["security"] as const;

type CollapsibleGroup = (typeof COLLAPSIBLE_GROUPS)[number];

export function collapsibleGroupOf(key: string): CollapsibleGroup | null {
  const prefix = groupOfKey(key);
  return COLLAPSIBLE_GROUPS.includes(prefix as CollapsibleGroup)
    ? (prefix as CollapsibleGroup)
    : null;
}

export interface EntryBlockRows<T> {
  group: CollapsibleGroup | null;
  rows: T[];
}

export function buildBlocks<T extends { key: string }>(
  rows: readonly T[],
): EntryBlockRows<T>[] {
  const blocks: EntryBlockRows<T>[] = [];
  const groupBlocks = new Map<CollapsibleGroup, EntryBlockRows<T>>();

  for (const row of rows) {
    const group = collapsibleGroupOf(row.key);
    if (group === null) {
      blocks.push({ group: null, rows: [row] });
      continue;
    }

    const existing = groupBlocks.get(group);
    if (existing !== undefined) {
      existing.rows.push(row);
      continue;
    }

    groupBlocks.set(group, { group, rows: [row] });
  }

  for (const group of COLLAPSIBLE_GROUPS) {
    const block = groupBlocks.get(group);
    if (block !== undefined) blocks.push(block);
  }

  return blocks;
}
