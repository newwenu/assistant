import type { ToolApprovalRequest } from "../../core/chatTypes";
import { buildDiagnostics } from "./diagnostics";
import { buildIntentViews, foldSections, selectReasons } from "./detail";
import {
  echoObject,
  evaluateBoundary,
  evaluateConsequences,
  evaluateObject,
  evaluateScale,
  evaluateVerb,
} from "./elements";
import { contentEvidence, diffEvidence, needsCommandBlock } from "./evidence";
import { DISCRIMINANT_KEYS } from "./tables";
import { argumentsText, commandText, isRecord } from "./text";
import type { ApprovalView, TranslateFn } from "./types";

/**
 * 对一条审批请求求值视图（I72 §5.1 顺序：动词 → 对象 → 规模 → 后果 → 边界）。
 * 空要素整段消失（N14），不留空槽与多余分隔符。
 */
export function buildApprovalView(
  approval: ToolApprovalRequest,
  t: TranslateFn,
): ApprovalView {
  const args = isRecord(approval.arguments) ? approval.arguments : {};
  const preview = approval.context.preview;
  const intents = approval.intents;
  const shell = preview?.shell ?? null;

  const isShell =
    shell !== null || intents.some((intent) => intent.action === "execute");
  const commandSubject = isShell ? (shell?.payload ?? commandText(args)) : null;
  const command =
    commandSubject !== null && needsCommandBlock(commandSubject)
      ? commandSubject
      : null;

  // 「已知工具」= 有后端事实（`intents` / `preview`）或命中判别参数（§5.2）。
  // 数据参数（`path` / `content` / `command` …）只描述形态、不产生语义，
  // 不能让未知工具借它冒充已知（§5.4 / A10）。
  const known =
    intents.length > 0 ||
    preview !== null ||
    DISCRIMINANT_KEYS.some((key) => args[key] !== undefined);

  if (!known) {
    // 未知工具：只回显标识，不产生语义（§5.4 / A10 / 禁令 3 显式豁免）。
    const verb = t("chat.tool.approval.verb.call");
    const object = echoObject(approval.toolName, args, t);
    const argumentsText_ = argumentsText(args);
    const diagnostics: ApprovalView["detail"]["diagnostics"] = {
      readOnlyAnomaly: approval.context.behavior.readOnly,
      previewSummary: null,
      previewUnavailable: false,
      extensions: [],
    };
    const reasons = selectReasons(approval.context.reasons);
    const alwaysIntents = `${verb} ${object}`;
    return {
      verb,
      object,
      scale: [],
      consequences: [],
      boundary: null,
      command: null,
      segments: [verb, object],
      scope: {
        session: t("chat.tool.approval.scope.session", {
          action: verb,
          object,
        }),
        always: t("chat.tool.approval.scope.always", {
          intents: alwaysIntents,
        }),
        alwaysIntents,
      },
      detail: {
        reasons,
        intents: [],
        evidence: null,
        arguments: argumentsText_,
        diagnostics,
        denyConsequence: t("chat.tool.approval.fold.deny_consequence"),
        sections: foldSections(reasons, [], null, diagnostics, argumentsText_),
      },
    };
  }

  const verb = evaluateVerb(args, preview, intents, t);
  const object = evaluateObject(
    approval.toolName,
    args,
    commandSubject,
    command,
    intents,
    t,
  );
  const scale = evaluateScale(args, preview, intents, isShell, t);
  const consequences = evaluateConsequences(
    args,
    preview,
    intents,
    isShell,
    commandSubject,
    t,
  );
  const boundary = evaluateBoundary(
    preview,
    approval.context.extensions,
    isShell,
    commandSubject,
    t,
  );

  const segments = [verb, object, ...scale, ...consequences].filter(
    (segment): segment is string => segment !== null && segment !== "",
  );

  const reasons = selectReasons(approval.context.reasons);
  const intentsView = buildIntentViews(intents, t);

  const oldString = args.old_string;
  const newString = args.new_string;
  const content = args.content;
  let evidence: ApprovalView["detail"]["evidence"] = null;
  if (typeof oldString === "string" && typeof newString === "string") {
    evidence = diffEvidence(oldString, newString, t);
  } else if (typeof content === "string") {
    evidence = contentEvidence(content, t);
  }

  const diagnostics = buildDiagnostics(
    approval.context.behavior,
    preview,
    approval.context.extensions,
    t,
  );
  if (
    typeof content === "string" &&
    preview !== null &&
    preview.exists === null
  ) {
    diagnostics.previewUnavailable = true;
  }

  const argumentsText_ = argumentsText(args);
  // 拼接单点：`always` 文案与二次确认共用同一份串，分隔符随语言（§8.2）。
  const alwaysIntents =
    intentsView.length > 0
      ? intentsView
          .map((intent) => `${intent.label} ${intent.head}`)
          .join(t("chat.tool.approval.scope.intent_separator"))
      : `${verb ?? ""} ${object ?? approval.toolName}`;

  return {
    verb,
    object,
    scale,
    consequences,
    boundary,
    command,
    segments,
    scope: {
      session: t("chat.tool.approval.scope.session", {
        action: verb ?? "",
        object: object ?? approval.toolName,
      }),
      always: t("chat.tool.approval.scope.always", { intents: alwaysIntents }),
      alwaysIntents,
    },
    detail: {
      reasons,
      intents: intentsView,
      evidence,
      arguments: argumentsText_,
      diagnostics,
      denyConsequence: t("chat.tool.approval.fold.deny_consequence"),
      sections: foldSections(
        reasons,
        intentsView,
        evidence,
        diagnostics,
        argumentsText_,
      ),
    },
  };
}
