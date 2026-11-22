import { translateKey, type I18nPath } from "@/i18n";
import type { ReclaimPolicyDto } from "@/types/protocol.contract";
import { formatBytes } from "@/utils/formatBytes";
import { createLogger } from "@/services/system/logger";

const storageLog = createLogger("storage");

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
  permissionDenied: "settings.storage.cleanup.error_code.permissionDenied",
  io: "settings.storage.cleanup.error_code.io",
  ownerUnavailable: "settings.storage.cleanup.error_code.ownerUnavailable",
  policyDisabled: "settings.storage.cleanup.error_code.policyDisabled",
  locked: "settings.storage.cleanup.error_code.locked",
  unknown: "settings.storage.cleanup.error_code.unknown",
} as const satisfies Record<string, I18nPath>;

const ENTRY_NAME_KEYS: Readonly<Record<string, I18nPath>> = {
  attachments: "settings.storage.entries.names.attachments",
  "attachments/derived": "settings.storage.entries.names.attachments_derived",
  "tool-results": "settings.storage.entries.names.tool_results",
  "tool-index": "settings.storage.entries.names.tool_index",
  "mascot-assets": "settings.storage.entries.names.mascot_assets",
  "assistant.db": "settings.storage.entries.names.assistant_db",
  "security/security.db": "settings.storage.entries.names.security_security_db",
  "security/backups": "settings.storage.entries.names.security_backups",
  "security/vault": "settings.storage.entries.names.security_vault",
  "security/recovery": "settings.storage.entries.names.security_recovery",
  "security/audit": "settings.storage.entries.names.security_audit",
  logs: "settings.storage.entries.names.logs",
  crashes: "settings.storage.entries.names.crashes",
  EBWebView: "settings.storage.entries.names.EBWebView",
  "config.json": "settings.storage.entries.names.config_json",
  "orb-config.json": "settings.storage.entries.names.orb_config_json",
  "mascot-config.json": "settings.storage.entries.names.mascot_config_json",
};

// 可选特性条目名称的自注册表：核心词表只认核心条目，
// 分区条目由 feature 在自己的模块里调用 `registerEntryNames` 登记（机制在核心、知识在特性）。
const FEATURE_ENTRY_NAME_KEYS: Record<string, I18nPath> = {};

export function registerEntryNames(
  keys: Readonly<Record<string, I18nPath>>,
): void {
  Object.assign(FEATURE_ENTRY_NAME_KEYS, keys);
}

const GROUP_NAME_KEYS: Readonly<Record<string, I18nPath>> = {
  security: "settings.storage.entries.groups.security",
};

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
    storageLog.warn(`unknown ${kind} value: "${value}"`);
    return value;
  }
  return translateKey(key);
}

function optionalTextOf(
  keys: Readonly<Record<string, I18nPath>>,
  value: string,
): string {
  const key = lookupKey(keys, value);
  return key === undefined ? value : translateKey(key);
}

export function entryNameText(key: string): string {
  const i18nKey =
    lookupKey(FEATURE_ENTRY_NAME_KEYS, key) ?? lookupKey(ENTRY_NAME_KEYS, key);
  return i18nKey === undefined ? key : translateKey(i18nKey);
}

export function groupNameText(group: string): string {
  return optionalTextOf(GROUP_NAME_KEYS, group);
}

export function availabilityText(value: string): string {
  return textOf(AVAILABILITY_KEYS, value, "availability");
}

export function eligibilityText(value: string): string {
  return textOf(ELIGIBILITY_KEYS, value, "eligibility");
}

export function reasonText(value: string): string {
  return textOf(REASON_KEYS, value, "cleanup reason");
}

export function actionText(value: string): string {
  return textOf(ACTION_KEYS, value, "cleanup action");
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

export function cleanupErrorCodeText(value: string): string {
  return textOf(ERROR_CODE_KEYS, value, "cleanup error code");
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
