import type {
  ApprovalExtension,
  ApprovalPreview,
  ToolApprovalRequest,
} from "../../core/chatTypes";
import { workspaceBasename } from "../../core/workspaceDisplay";
import {
  formatAction,
  INTENT_ACTION,
  summarizeArguments,
} from "../toolCallDisplay";
import {
  COMMAND_WIDTH_LIMIT,
  EXTENSION_KIND,
  RESOURCE_WIDTH_LIMIT,
  VERB_CODE,
} from "./tables";
import {
  formatBytes,
  hasControlOperator,
  lineCount,
  middleEllipsis,
  nonEmptyString,
  readBoolean,
  utf8Length,
} from "./text";
import type { ApprovalBoundary, TranslateFn } from "./types";

/** §5.1 决策要素求值：动词 / 对象 / 规模 / 后果 / 边界 + 取词。 */

/** 首屏资源：路径取末段；`execute` 是命令原文；`mcp` 只留 server（§8.2）。 */
export function resourceHead(action: string, resource: string): string {
  if (action === "execute")
    return middleEllipsis(resource, RESOURCE_WIDTH_LIMIT);
  if (action === "mcp") {
    const parts = resource.split(":");
    return parts.length >= 3 && parts[1] ? parts[1] : resource;
  }
  return workspaceBasename(resource);
}

/** 未知 action 原样回显并标「未识别」（§8.2）。 */
export function actionLabel(action: string, t: TranslateFn): string {
  const label = formatAction(action, t);
  if (INTENT_ACTION[action]) return label;
  return t("chat.tool.approval.intent.unrecognized", { action: label });
}

/** 未知工具的对象段：`toolName` + 参数摘要，只回显标识、不产生语义（§5.4）。 */
export function echoObject(
  toolName: string,
  args: Record<string, unknown>,
  t: TranslateFn,
): string {
  const summary = summarizeArguments(args);
  return summary === null
    ? toolName
    : t("chat.tool.approval.object.echo", { tool: toolName, summary });
}

/** `append=false` / 缺省：动词只由 `preview.exists` 决定，不猜（§5.2）。 */
function writeVerbByExists(
  preview: ApprovalPreview | null,
  t: TranslateFn,
): string {
  const exists = preview?.exists;
  if (exists === false) return t("chat.tool.approval.verb.create");
  if (exists === true) return t("chat.tool.approval.verb.overwrite");
  return t("chat.tool.approval.verb.create_or_overwrite");
}

export function evaluateVerb(
  args: Record<string, unknown>,
  preview: ApprovalPreview | null,
  intents: ToolApprovalRequest["intents"],
  t: TranslateFn,
): string | null {
  const permanent = readBoolean(args, "permanent");
  const recursive = readBoolean(args, "recursive");
  const append = readBoolean(args, "append");
  const source = nonEmptyString(args.source);
  const target = nonEmptyString(args.target);

  // `source` + `target` 只在无其它判别参数时确定为「移动」（§5.2 / D10 / A10c）。
  if (source !== null && target !== null) {
    if (
      args.permanent !== undefined ||
      args.recursive !== undefined ||
      args.append !== undefined
    ) {
      return null;
    }
    return t("chat.tool.approval.verb.move");
  }

  if (permanent !== undefined || recursive !== undefined) {
    if (permanent === true && recursive === true) {
      return t("chat.tool.approval.verb.delete_dir_permanent");
    }
    if (permanent === true)
      return t("chat.tool.approval.verb.delete_permanent");
    if (recursive === true) return t("chat.tool.approval.verb.delete_dir");
    return t("chat.tool.approval.verb.delete");
  }

  if (append !== undefined) {
    if (append) return t("chat.tool.approval.verb.append");
    return writeVerbByExists(preview, t);
  }

  // `append` 缺省时后端按 `false` 处理；`content` 是唯一可证的写入形态（§5.2）。
  if (typeof args.content === "string") {
    return writeVerbByExists(preview, t);
  }

  const verbCode = preview?.verbCode;
  if (verbCode) {
    const key = VERB_CODE[verbCode];
    if (key) return t(key);
  }

  const action = intents[0]?.action;
  if (action) return actionLabel(action, t);
  return t("chat.tool.approval.verb.call");
}

/** 对象段：命令即对象；否则路径末段 / `source → target` / `intents` 末段 / 回显（§5.1）。 */
export function evaluateObject(
  toolName: string,
  args: Record<string, unknown>,
  commandSubject: string | null,
  commandBlock: string | null,
  intents: ToolApprovalRequest["intents"],
  t: TranslateFn,
): string | null {
  const source = nonEmptyString(args.source);
  const target = nonEmptyString(args.target);
  const path = nonEmptyString(args.path);
  if (commandSubject !== null) {
    return commandBlock === null
      ? middleEllipsis(commandSubject, COMMAND_WIDTH_LIMIT)
      : null;
  }
  if (path !== null) return workspaceBasename(path);
  if (source !== null && target !== null) {
    return t("chat.tool.approval.object.pair", {
      source: workspaceBasename(source),
      target: workspaceBasename(target),
    });
  }
  if (intents.length > 0) {
    const first = intents[0];
    const resource = first.resources[0];
    return first.action === "mcp" || resource === undefined
      ? echoObject(toolName, args, t)
      : resourceHead(first.action, resource);
  }
  return echoObject(toolName, args, t);
}

export function evaluateConsequences(
  args: Record<string, unknown>,
  preview: ApprovalPreview | null,
  intents: ToolApprovalRequest["intents"],
  isShell: boolean,
  command: string | null,
  t: TranslateFn,
): string[] {
  if (isShell) {
    const shell = preview?.shell ?? null;
    const control = command !== null && hasControlOperator(command);
    // R2：含控制操作符时不得给出「只读」结论。
    const effective =
      shell === null
        ? "unknown"
        : shell.class === "readOnly" && control
          ? "unknown"
          : shell.class;
    if (effective === "readOnly") {
      return [t("chat.tool.approval.consequence.shell_read_only")];
    }
    if (effective === "network") {
      return [t("chat.tool.approval.consequence.shell_network")];
    }
    if (effective === "destructive") {
      return [t("chat.tool.approval.consequence.shell_destructive")];
    }
    return [t("chat.tool.approval.consequence.shell_unknown")];
  }

  const isDelete = intents.some(
    (intent) => intent.action === "delete" || intent.action === "purge",
  );
  if (isDelete) {
    const permanent =
      readBoolean(args, "permanent") === true ||
      intents.some((intent) => intent.action === "purge");
    const out = [
      t(
        permanent
          ? "chat.tool.approval.consequence.unrecoverable"
          : "chat.tool.approval.consequence.recoverable",
      ),
    ];
    if (preview?.symlink === true) {
      out.push(t("chat.tool.approval.consequence.link_only"));
    }
    return out;
  }

  const append = readBoolean(args, "append");
  if (append === true) {
    return [t("chat.tool.approval.consequence.append_keep")];
  }
  if (typeof args.content === "string" && preview?.exists === true) {
    return [t("chat.tool.approval.consequence.overwritten")];
  }

  const overwrite = readBoolean(args, "overwrite");
  if (
    nonEmptyString(args.source) !== null &&
    nonEmptyString(args.target) !== null
  ) {
    return [
      t(
        overwrite === true
          ? "chat.tool.approval.consequence.overwrite_if_exists"
          : "chat.tool.approval.consequence.no_overwrite",
      ),
    ];
  }
  return [];
}

export function evaluateScale(
  args: Record<string, unknown>,
  preview: ApprovalPreview | null,
  intents: ToolApprovalRequest["intents"],
  isShell: boolean,
  t: TranslateFn,
): string[] {
  if (isShell) {
    const shell = preview?.shell ?? null;
    if (shell === null) return [];
    const out: string[] = [];
    if (shell.statements >= 2) {
      out.push(
        t("chat.tool.approval.scale.statements", { count: shell.statements }),
      );
    }
    if (shell.redirects.length > 0) {
      out.push(
        t("chat.tool.approval.scale.redirect", {
          target: shell.redirects
            .map((path) => workspaceBasename(path))
            .join(", "),
        }),
      );
    }
    return out;
  }

  const oldString = args.old_string;
  const newString = args.new_string;
  if (typeof oldString === "string" && typeof newString === "string") {
    return [
      t("chat.tool.approval.scale.lines_transition", {
        from: lineCount(oldString),
        to: lineCount(newString),
      }),
    ];
  }

  const startLine = args.startLine;
  const endLine = args.endLine;
  const replacement = args.replacement;
  if (typeof startLine === "number" && typeof replacement === "string") {
    const last = typeof endLine === "number" ? endLine : startLine;
    return [
      t("chat.tool.approval.scale.range_transition", {
        from: startLine,
        to: last,
        lines: lineCount(replacement),
      }),
    ];
  }

  const content = args.content;
  if (typeof content === "string") {
    const append = readBoolean(args, "append");
    if (append === true) {
      return [
        t("chat.tool.approval.scale.new_lines", { count: lineCount(content) }),
      ];
    }
    return [
      t("chat.tool.approval.scale.lines_bytes", {
        lines: lineCount(content),
        bytes: formatBytes(utf8Length(content)),
      }),
    ];
  }

  const isDelete = intents.some(
    (intent) => intent.action === "delete" || intent.action === "purge",
  );
  if (!isDelete) return [];

  const recursive = readBoolean(args, "recursive") === true;
  const targetKind = preview?.targetKind ?? null;
  const entries = preview?.entries ?? null;
  const bytes = preview?.bytes ?? null;
  if (targetKind === "link") return [];
  const out: string[] = [];
  if ((recursive || targetKind === "directory") && entries !== null) {
    out.push(t("chat.tool.approval.scale.entries", { count: entries }));
  }
  if (bytes !== null) {
    out.push(
      t("chat.tool.approval.scale.bytes", { bytes: formatBytes(bytes) }),
    );
  }
  return out;
}

export function evaluateBoundary(
  preview: ApprovalPreview | null,
  extensions: ApprovalExtension[],
  isShell: boolean,
  command: string | null,
  t: TranslateFn,
): ApprovalBoundary | null {
  if (isShell) {
    const shell = preview?.shell ?? null;
    if (shell === null) return null;
    const control = command !== null && hasControlOperator(command);
    // 只读且无控制操作符：无可写目标，边界无意义（§6.5）。
    if (shell.class === "readOnly" && !control) return null;
    if (shell.targetsScope === "inside") {
      return {
        text: t("chat.tool.approval.boundary.shell_inside"),
        severity: "caution",
      };
    }
    if (shell.targetsScope === "outside") {
      return {
        text: t("chat.tool.approval.boundary.shell_outside"),
        severity: "caution",
      };
    }
    return null;
  }

  const workspace = extensions.find(
    (extension) =>
      extension.kind === "workspace" && extension.severity === "caution",
  );
  if (!workspace) return null;
  const key = EXTENSION_KIND[`${workspace.kind}.${workspace.code}`];
  return {
    text: key ? t(key) : `${workspace.kind}.${workspace.code}`,
    severity: "caution",
  };
}
