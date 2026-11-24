/**
 * 审批卡片视图模型（I72 §5）的对外入口。
 *
 * 三层分离：内容由数据（`arguments` / `intents` / `preview` / `extensions`）给出，
 * 语义由参数语义字典给出，呈现由组件给出。整组模块是纯函数：不依赖 React、
 * 不接收工作区根、不按 `toolName` 分支（`TOOL_PROFILES` 查表与未知工具回显除外）。
 *
 * 拆分（`docs/principle/CODE-SPLIT-PRINCIPLE.md`）：
 * - `types.ts`       视图契约
 * - `tables.ts`      四张纯表、字典键集合、阈值、优先级
 * - `text.ts`        叶子纯函数（行 / 字节 / 显示宽度 / 中间省略 / 参数原文）
 * - `elements.ts`    §5.1 要素求值 + 取词
 * - `evidence.ts`    §5.3 证据块与命令块阈值
 * - `diagnostics.ts` N7 诊断与降级
 * - `detail.ts`      §7.2 原因选取、§8.2 授权范围、§4.5 区块顺序
 * - `view.ts`        `buildApprovalView` 组合入口
 */

export { buildApprovalView } from "./view";
export {
  actionLabel,
  echoObject,
  evaluateBoundary,
  evaluateConsequences,
  evaluateObject,
  evaluateScale,
  evaluateVerb,
  resourceHead,
} from "./elements";
export {
  COMMAND_WIDTH_LIMIT,
  CONTENT_MAX_BYTES,
  CONTENT_MAX_LINES,
  DIFF_MAX_LINES,
  DISCRIMINANT_KEYS,
  EXTENSION_KIND,
  EXTENSION_SEVERITY_RANK,
  INTENT_ACTION,
  REASON_PRIORITY,
  RESOURCE_WIDTH_LIMIT,
  TOOLBOX_ONLY_IDS,
  TOOL_PROFILES,
  VERB_CODE,
} from "./tables";
export { buildIntentViews, foldSections, selectReasons } from "./detail";
export { buildDiagnostics } from "./diagnostics";
export { contentEvidence, diffEvidence, needsCommandBlock } from "./evidence";
export { argumentsText, commandText } from "./text";
export type {
  ApprovalBoundary,
  ApprovalDetail,
  ApprovalDiagnostics,
  ApprovalEvidence,
  ApprovalEvidenceKind,
  ApprovalExtensionView,
  ApprovalFoldSectionId,
  ApprovalIntentView,
  ApprovalReasons,
  ApprovalScope,
  ApprovalView,
  TranslateFn,
} from "./types";
