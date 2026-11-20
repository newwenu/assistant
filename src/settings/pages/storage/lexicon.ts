import { translateKey, type I18nPath } from "@/i18n";
import type { ReclaimPolicyDto } from "@/types/protocol.contract";
import { formatBytes } from "@/utils/formatBytes";

export const ENTRY_KEYS = [
  "attachments",
  "attachments/derived",
  "tool-results",
  "tool-index",
  "mascot-assets",
  "assistant.db",
  "security/security.db",
  "security/backups",
  "security/vault",
  "security/recovery",
  "security/audit",
  "logs",
  "crashes",
] as const;

export type EntryKey = (typeof ENTRY_KEYS)[number];

const ENTRY_IDS = {
  attachments: "E1",
  "attachments/derived": "E2",
  "tool-results": "E3",
  "tool-index": "E4",
  "mascot-assets": "E5",
  "assistant.db": "E6",
  "security/security.db": "E7",
  "security/backups": "E8",
  "security/vault": "E9",
  "security/recovery": "E10",
  "security/audit": "E11",
  logs: "E12",
  crashes: "E13",
} as const satisfies Record<EntryKey, string>;

const ENTRY_NAME_KEYS = {
  attachments: "settings.storage.entries.names.e1",
  "attachments/derived": "settings.storage.entries.names.e2",
  "tool-results": "settings.storage.entries.names.e3",
  "tool-index": "settings.storage.entries.names.e4",
  "mascot-assets": "settings.storage.entries.names.e5",
  "assistant.db": "settings.storage.entries.names.e6",
  "security/security.db": "settings.storage.entries.names.e7",
  "security/backups": "settings.storage.entries.names.e8",
  "security/vault": "settings.storage.entries.names.e9",
  "security/recovery": "settings.storage.entries.names.e10",
  "security/audit": "settings.storage.entries.names.e11",
  logs: "settings.storage.entries.names.e12",
  crashes: "settings.storage.entries.names.e13",
} as const satisfies Record<EntryKey, I18nPath>;

const AVAILABILITY_KEYS = {
  available: "settings.storage.entries.availability.available",
  missing: "settings.storage.entries.availability.missing",
  locked: "settings.storage.entries.availability.locked",
  unreadable: "settings.storage.entries.availability.unreadable",
  unavailable: "settings.storage.entries.availability.unavailable",
  notImplemented: "settings.storage.entries.availability.notImplemented",
  noReportYet: "settings.storage.entries.availability.noReportYet",
  staleReport: "settings.storage.entries.availability.staleReport",
} as const satisfies Record<string, I18nPath>;

const ELIGIBILITY_KEYS = {
  eligible: "settings.storage.entries.eligibility.eligible",
  notEligible: "settings.storage.entries.eligibility.notEligible",
  locked: "settings.storage.entries.eligibility.locked",
  ownerManaged: "settings.storage.entries.eligibility.ownerManaged",
  notImplemented: "settings.storage.entries.eligibility.notImplemented",
} as const satisfies Record<string, I18nPath>;

const REASON_KEYS = {
  orphan: "settings.storage.entries.reason.orphan",
  expired: "settings.storage.entries.reason.expired",
  overLimit: "settings.storage.entries.reason.overLimit",
  derivedCache: "settings.storage.entries.reason.derivedCache",
  transient: "settings.storage.entries.reason.transient",
  referenced: "settings.storage.entries.reason.referenced",
  userData: "settings.storage.entries.reason.userData",
  businessData: "settings.storage.entries.reason.businessData",
  replayRisk: "settings.storage.entries.reason.replayRisk",
  locked: "settings.storage.entries.reason.locked",
  noStats: "settings.storage.entries.reason.noStats",
  noReclaimApi: "settings.storage.entries.reason.noReclaimApi",
  externalOwner: "settings.storage.entries.reason.externalOwner",
  policyDisabled: "settings.storage.entries.reason.policyDisabled",
  unknown: "settings.storage.entries.reason.unknown",
} as const satisfies Record<string, I18nPath>;

const ACTION_KEYS = {
  none: "settings.storage.entries.action.none",
  deleteOrphans: "settings.storage.entries.action.deleteOrphans",
  deleteSelected: "settings.storage.entries.action.deleteSelected",
  sweepExpired: "settings.storage.entries.action.sweepExpired",
  pruneToLimit: "settings.storage.entries.action.pruneToLimit",
  rebuildCache: "settings.storage.entries.action.rebuildCache",
  rotate: "settings.storage.entries.action.rotate",
} as const satisfies Record<string, I18nPath>;

const POLICY_SOURCE_KEYS = {
  config: "settings.storage.entries.policy.source.config",
  constant: "settings.storage.entries.policy.source.constant",
  owner: "settings.storage.entries.policy.source.owner",
  none: "settings.storage.entries.policy.source.none",
} as const satisfies Record<string, I18nPath>;

const INVENTORY_CLASS_KEYS = {
  declared: "settings.storage.residual.class.declared",
  optionalNotMounted: "settings.storage.residual.class.optionalNotMounted",
  companion: "settings.storage.residual.class.companion",
  ignored: "settings.storage.residual.class.ignored",
  unregistered: "settings.storage.residual.class.unregistered",
} as const satisfies Record<string, I18nPath>;

const LAYER_KEYS = {
  data: "settings.storage.entries.layer.data",
  local: "settings.storage.entries.layer.local",
} as const satisfies Record<string, I18nPath>;

const ERROR_CODE_KEYS = {
  permissionDenied: "settings.storage.reclaim.error_code.permissionDenied",
  io: "settings.storage.reclaim.error_code.io",
  ownerUnavailable: "settings.storage.reclaim.error_code.ownerUnavailable",
  policyDisabled: "settings.storage.reclaim.error_code.policyDisabled",
  locked: "settings.storage.reclaim.error_code.locked",
  unknown: "settings.storage.reclaim.error_code.unknown",
} as const satisfies Record<string, I18nPath>;

function lookupKey(
  keys: Readonly<Record<string, I18nPath>>,
  value: string,
): I18nPath | undefined {
  return Object.prototype.hasOwnProperty.call(keys, value)
    ? keys[value]
    : undefined;
}

function textOf(
  keys: Readonly<Record<string, I18nPath>>,
  value: string,
  kind: string,
): string {
  const key = lookupKey(keys, value);
  if (key === undefined) {
    console.warn(`[storage] unknown ${kind} value: "${value}"`);
    return value;
  }
  return translateKey(key);
}

export function entryIdText(key: string): string | null {
  return Object.prototype.hasOwnProperty.call(ENTRY_IDS, key)
    ? ENTRY_IDS[key as EntryKey]
    : null;
}

export function entryNameText(key: string): string {
  const id = entryIdText(key);
  if (id === null) {
    console.warn(`[storage] unknown entry key: "${key}"`);
    return key;
  }
  return translateKey(ENTRY_NAME_KEYS[key as EntryKey]);
}

export function availabilityText(value: string): string {
  return textOf(AVAILABILITY_KEYS, value, "availability");
}

export function eligibilityText(value: string): string {
  return textOf(ELIGIBILITY_KEYS, value, "eligibility");
}

export function reasonText(value: string): string {
  return textOf(REASON_KEYS, value, "reclaim reason");
}

export function actionText(value: string): string {
  return textOf(ACTION_KEYS, value, "reclaim action");
}

export function policySourceText(value: string): string {
  return textOf(POLICY_SOURCE_KEYS, value, "policy source");
}

export function inventoryClassText(value: string): string {
  return textOf(INVENTORY_CLASS_KEYS, value, "inventory class");
}

export function layerText(value: string): string {
  return textOf(LAYER_KEYS, value, "layer");
}

export function reclaimErrorCodeText(value: string): string {
  return textOf(ERROR_CODE_KEYS, value, "reclaim error code");
}

export function policyChips(
  policy: ReclaimPolicyDto,
  autoActive: boolean,
): string[] {
  const chips: string[] = [policySourceText(policy.source)];

  if (policy.retentionDays !== null) {
    chips.push(
      policy.retentionDays === 0
        ? translateKey("settings.storage.entries.policy.retention_disabled")
        : translateKey("settings.storage.entries.policy.retention", {
            days: policy.retentionDays,
          }),
    );
  }
  if (policy.keepCount !== null) {
    chips.push(
      translateKey("settings.storage.entries.policy.keep_count", {
        count: policy.keepCount,
      }),
    );
  }
  if (policy.maxBytes !== null) {
    chips.push(
      translateKey("settings.storage.entries.policy.max_bytes", {
        size: formatBytes(policy.maxBytes),
      }),
    );
  }
  if (autoActive) {
    chips.push(translateKey("settings.storage.entries.policy.auto_active"));
  }

  return chips;
}
