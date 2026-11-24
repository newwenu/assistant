import type {
  ApprovalExtensionSeverity,
  ApprovalReason,
} from "../../core/chatTypes";

/** 审批卡片视图模型（I72 §5）。 */

export type TranslateFn = (
  key: string,
  options?: Record<string, unknown>,
) => string;

/** 行 2 证据块形态（I72 §5.3）。 */
export type ApprovalEvidenceKind = "command" | "diff" | "content";

export interface ApprovalEvidence {
  kind: ApprovalEvidenceKind;
  text: string;
  truncated: boolean;
}

/** 选取归视图模型、文案归卡片（§7.1）：这里只产出原始原因，不产出本地化文本。 */
export interface ApprovalReasons {
  primary: ApprovalReason | null;
  rest: ApprovalReason[];
}

export interface ApprovalIntentView {
  action: string;
  label: string;
  /** 首屏口径：路径末段 / 命令原文 / `mcp` 的 server 段。 */
  head: string;
  /** 折叠区口径：绝对路径 / 命令原文 / 完整 `mcp:` 串。 */
  raw: string[];
}

export interface ApprovalExtensionView {
  kind: string;
  code: string;
  severity: ApprovalExtensionSeverity;
  label: string;
}

export interface ApprovalDiagnostics {
  /** `behavior.readOnly = true`：只读工具被要求审批本身就是异常（§3.2 / N7）。 */
  readOnlyAnomaly: boolean;
  /** 折叠区诊断原文（标注已前置）；`null` = 无正文可渲染（§5.3）。 */
  previewSummary: string | null;
  /** `write` 的 `exists = null`：预演降级（§5.6.3）。 */
  previewUnavailable: boolean;
  extensions: ApprovalExtensionView[];
}

export type ApprovalFoldSectionId =
  | "why"
  | "scope"
  | "evidence"
  | "arguments"
  | "diagnostics"
  | "denyConsequence";

export interface ApprovalDetail {
  reasons: ApprovalReasons;
  intents: ApprovalIntentView[];
  evidence: ApprovalEvidence | null;
  arguments: string;
  diagnostics: ApprovalDiagnostics;
  denyConsequence: string;
  /** §4.5 固定顺序；入口文案按此列表拼接。 */
  sections: ApprovalFoldSectionId[];
}

export interface ApprovalBoundary {
  text: string;
  severity: "caution";
}

export interface ApprovalScope {
  session: string;
  always: string;
  /** `always` 的 `{{intents}}` 实参：二次确认复用同一份串，卡片不再自行拼接。 */
  alwaysIntents: string;
}

export interface ApprovalView {
  /** 行 1 首段（加粗、最左）；`null` = 未知工具外的空要素。 */
  verb: string | null;
  object: string | null;
  scale: string[];
  consequences: string[];
  boundary: ApprovalBoundary | null;
  /** 行 2：命令块全文；`null` = 无证据块。 */
  command: string | null;
  /**
   * 行 1 文本段（动词 → 对象 → 规模 → 后果），已剔除空要素。
   * 边界告警需图标呈现，单独由 `boundary` 承载，不在本数组内。
   */
  segments: string[];
  scope: ApprovalScope;
  detail: ApprovalDetail;
}
