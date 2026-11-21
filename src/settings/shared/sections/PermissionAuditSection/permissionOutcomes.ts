import { translateKey, type I18nPath } from "@/i18n";

export const PERMISSION_AUDIT_OUTCOMES = [
  "allowed",
  "grantHit",
  "denied",
  "approved",
  "sessionApproved",
  "alwaysApproved",
  "rejected",
  "timedOut",
  "validateRejected",
  "intentError",
  "bypassed",
] as const;

export type PermissionAuditOutcome = (typeof PERMISSION_AUDIT_OUTCOMES)[number];

const OUTCOME_TEXT_KEYS = {
  allowed: "settings.agent_mode.audit.outcome.allowed",
  grantHit: "settings.agent_mode.audit.outcome.grantHit",
  denied: "settings.agent_mode.audit.outcome.denied",
  approved: "settings.agent_mode.audit.outcome.approved",
  sessionApproved: "settings.agent_mode.audit.outcome.sessionApproved",
  alwaysApproved: "settings.agent_mode.audit.outcome.alwaysApproved",
  rejected: "settings.agent_mode.audit.outcome.rejected",
  timedOut: "settings.agent_mode.audit.outcome.timedOut",
  validateRejected: "settings.agent_mode.audit.outcome.validateRejected",
  intentError: "settings.agent_mode.audit.outcome.intentError",
  bypassed: "settings.agent_mode.audit.outcome.bypassed",
} as const satisfies Record<PermissionAuditOutcome, I18nPath>;

function lookupKey(
  keys: Readonly<Record<string, I18nPath>>,
  value: string,
): I18nPath | undefined {
  return Object.prototype.hasOwnProperty.call(keys, value)
    ? keys[value]
    : undefined;
}

export function permissionAuditOutcomeText(outcome: string): string {
  const key = lookupKey(OUTCOME_TEXT_KEYS, outcome);
  return key === undefined ? outcome : translateKey(key);
}
