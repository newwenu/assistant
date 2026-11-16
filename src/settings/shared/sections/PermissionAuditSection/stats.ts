import type { PermissionAuditEntry } from "@/services/tool/permissionService";

const ADMIN_TOOLS = new Set(["workspace", "grant"]);
const TOOLBOX_RESOURCE = "toolbox";

const POLICY_OUTCOMES = new Set([
  "allowed",
  "grantHit",
  "denied",
  "approved",
  "sessionApproved",
  "alwaysApproved",
  "rejected",
  "timedOut",
]);
const APPROVED_OUTCOMES = new Set([
  "approved",
  "sessionApproved",
  "alwaysApproved",
]);
const ASKED_OUTCOMES = new Set([...APPROVED_OUTCOMES, "rejected", "timedOut"]);

const OUTCOME_COLORS: Record<string, string> = {
  allowed: "green",
  grantHit: "green",
  approved: "blue",
  sessionApproved: "blue",
  alwaysApproved: "blue",
  denied: "red",
  rejected: "red",
  validateRejected: "red",
  intentError: "red",
  timedOut: "orange",
};

export interface AuditStats {
  decisions: number;
  policy: number;
  denied: number;
  asked: number;
  approved: number;
}

function isDecisionRow(row: PermissionAuditEntry): boolean {
  return !ADMIN_TOOLS.has(row.tool) && row.resource !== TOOLBOX_RESOURCE;
}

export function computeStats(rows: PermissionAuditEntry[]): AuditStats {
  const stats: AuditStats = {
    decisions: 0,
    policy: 0,
    denied: 0,
    asked: 0,
    approved: 0,
  };
  for (const row of rows) {
    if (!isDecisionRow(row)) continue;
    stats.decisions += 1;
    if (!POLICY_OUTCOMES.has(row.outcome)) continue;
    stats.policy += 1;
    if (row.outcome === "denied") stats.denied += 1;
    if (ASKED_OUTCOMES.has(row.outcome)) {
      stats.asked += 1;
      if (APPROVED_OUTCOMES.has(row.outcome)) stats.approved += 1;
    }
  }
  return stats;
}

export function percent(numerator: number, denominator: number): string {
  if (denominator === 0) return "—";
  return `${Math.round((numerator / denominator) * 100)}%`;
}

export function outcomeColor(outcome: string): string | undefined {
  return Object.prototype.hasOwnProperty.call(OUTCOME_COLORS, outcome)
    ? OUTCOME_COLORS[outcome]
    : undefined;
}
