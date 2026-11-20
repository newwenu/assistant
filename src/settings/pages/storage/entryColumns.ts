import type { I18nPath } from "@/i18n";
import { entryIdText } from "./lexicon";

export type EntryColumnKey =
  "name" | "size" | "availability" | "eligibility" | "policy" | "action";

export const ENTRY_COLUMN_ORDER: readonly EntryColumnKey[] = [
  "name",
  "size",
  "availability",
  "eligibility",
  "policy",
  "action",
];

export const ENTRY_COLUMN_MIN_PX = {
  name: 220,
  size: 120,
  availability: 120,
  eligibility: 176,
  policy: 152,
  action: 88,
} as const satisfies Record<EntryColumnKey, number>;

export const ENTRY_COLUMN_WIDTHS = {
  name: `minmax(${ENTRY_COLUMN_MIN_PX.name}px, 1.4fr)`,
  size: `${ENTRY_COLUMN_MIN_PX.size}px`,
  availability: `${ENTRY_COLUMN_MIN_PX.availability}px`,
  eligibility: `${ENTRY_COLUMN_MIN_PX.eligibility}px`,
  policy: `${ENTRY_COLUMN_MIN_PX.policy}px`,
  action: `${ENTRY_COLUMN_MIN_PX.action}px`,
} as const satisfies Record<EntryColumnKey, string>;

export const ENTRY_GRID_TEMPLATE = ENTRY_COLUMN_ORDER.map(
  (key) => ENTRY_COLUMN_WIDTHS[key],
).join(" ");

export const ENTRY_ROW_GAP_PX = 12;

export const ENTRY_ROW_PADDING_X_PX = 12;

export const ENTRY_TABLE_MIN_WIDTH =
  ENTRY_COLUMN_ORDER.reduce((sum, key) => sum + ENTRY_COLUMN_MIN_PX[key], 0) +
  ENTRY_ROW_GAP_PX * (ENTRY_COLUMN_ORDER.length - 1) +
  ENTRY_ROW_PADDING_X_PX * 2;

export const ENTRY_COLUMN_LABEL_KEYS = {
  name: "settings.storage.entries.columns.name",
  size: "settings.storage.entries.columns.size",
  availability: "settings.storage.entries.columns.availability",
  eligibility: "settings.storage.entries.columns.eligibility",
  policy: "settings.storage.entries.columns.policy",
  action: "settings.storage.entries.columns.action",
} as const satisfies Record<EntryColumnKey, I18nPath>;

export type EntryDomain =
  | "attachments"
  | "tools"
  | "mascot"
  | "database"
  | "security"
  | "diagnostics"
  | "other";

export const ENTRY_DOMAIN_ORDER: readonly Exclude<EntryDomain, "other">[] = [
  "attachments",
  "tools",
  "mascot",
  "database",
  "security",
  "diagnostics",
];

export const ENTRY_DOMAIN_IDS = {
  attachments: ["E1", "E2"],
  tools: ["E3", "E4"],
  mascot: ["E5"],
  database: ["E6"],
  security: ["E7", "E8", "E9", "E10", "E11"],
  diagnostics: ["E12", "E13"],
} as const satisfies Record<Exclude<EntryDomain, "other">, readonly string[]>;

export const ENTRY_DOMAIN_LABEL_KEYS = {
  attachments: "settings.storage.entries.domains.attachments",
  tools: "settings.storage.entries.domains.tools",
  mascot: "settings.storage.entries.domains.mascot",
  database: "settings.storage.entries.domains.database",
  security: "settings.storage.entries.domains.security",
  diagnostics: "settings.storage.entries.domains.diagnostics",
  other: "settings.storage.entries.domains.other",
} as const satisfies Record<EntryDomain, I18nPath>;

export function domainOfKey(key: string): EntryDomain {
  const id = entryIdText(key);
  if (id === null) return "other";
  for (const domain of ENTRY_DOMAIN_ORDER) {
    const ids: readonly string[] = ENTRY_DOMAIN_IDS[domain];
    if (ids.includes(id)) return domain;
  }
  return "other";
}
