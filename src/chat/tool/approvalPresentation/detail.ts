import type { ApprovalReason, ToolApprovalRequest } from "../../core/chatTypes";
import { actionLabel, resourceHead } from "./elements";
import { REASON_PRIORITY } from "./tables";
import type {
  ApprovalDiagnostics,
  ApprovalEvidence,
  ApprovalFoldSectionId,
  ApprovalIntentView,
  ApprovalReasons,
  TranslateFn,
} from "./types";

/** 折叠区内容：§7.2 原因选取、§8.2 授权范围、§4.5 区块顺序。 */

/** 去重（同一 `rule` 串重复只留一次）+ 按优先级取主句，其余进 `rest`（§7.2）。 */
export function selectReasons(reasons: ApprovalReason[]): ApprovalReasons {
  const unique: ApprovalReason[] = [];
  const seen = new Set<string>();
  for (const reason of reasons) {
    const key = JSON.stringify(reason);
    if (seen.has(key)) continue;
    seen.add(key);
    unique.push(reason);
  }
  const sorted = unique
    .map((reason) => ({ reason, priority: REASON_PRIORITY[reason.kind] }))
    .sort((a, b) => a.priority - b.priority)
    .map(({ reason }) => reason);
  const primary = sorted.length > 0 ? sorted[0] : null;
  return { primary, rest: sorted.slice(1) };
}

/** 折叠区「授权范围」：同 action 的资源合并，首屏末段、折叠区原文（§8.2 / N15）。 */
export function buildIntentViews(
  intents: ToolApprovalRequest["intents"],
  t: TranslateFn,
): ApprovalIntentView[] {
  const grouped: ApprovalIntentView[] = [];
  for (const intent of intents) {
    const label = actionLabel(intent.action, t);
    const existing = grouped.find((item) => item.action === intent.action);
    if (existing) {
      existing.raw.push(...intent.resources);
      const head = intent.resources
        .map((resource) => resourceHead(intent.action, resource))
        .join(", ");
      existing.head = existing.head === "" ? head : `${existing.head}, ${head}`;
      continue;
    }
    grouped.push({
      action: intent.action,
      label,
      head: intent.resources
        .map((resource) => resourceHead(intent.action, resource))
        .join(", "),
      raw: [...intent.resources],
    });
  }
  return grouped;
}

/** §4.5 固定顺序，每项有内容才出现（N14）。 */
export function foldSections(
  reasons: ApprovalReasons,
  intents: ApprovalIntentView[],
  evidence: ApprovalEvidence | null,
  diagnostics: ApprovalDiagnostics,
  arguments_: string,
): ApprovalFoldSectionId[] {
  const sections: ApprovalFoldSectionId[] = [];
  if (reasons.primary !== null || reasons.rest.length > 0) sections.push("why");
  if (intents.length > 0) sections.push("scope");
  if (evidence !== null) sections.push("evidence");
  if (arguments_ !== "{}") sections.push("arguments");
  if (
    diagnostics.readOnlyAnomaly ||
    diagnostics.previewSummary !== null ||
    diagnostics.previewUnavailable ||
    diagnostics.extensions.length > 0
  ) {
    sections.push("diagnostics");
  }
  sections.push("denyConsequence");
  return sections;
}
