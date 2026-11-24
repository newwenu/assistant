import type {
  ApprovalExtensionSeverity,
  ApprovalReason,
} from "../../core/chatTypes";

/** 四张纯表与阈值（I72 §3.1 / §5.2 / §5.5 / §8.2）。 */

/** `TOOLBOX_ONLY` 工具：不产生审批卡片（§2.5），不进入 `TOOL_PROFILES`。 */
export const TOOLBOX_ONLY_IDS: ReadonlySet<string> = new Set(["fs_link"]);

/**
 * 恒定字段声明（I72 §5.5）：只回答「该 `behavior` 字段对本工具是否恒定」，
 * 不声明显示哪些行。`readOnly=true` 不进表——只读工具被要求审批是异常诊断。
 */
export const TOOL_PROFILES: Record<
  string,
  Readonly<{ destructive?: true; openWorld?: true }>
> = {
  delete: { destructive: true },
  shell: { destructive: true, openWorld: true },
  super_edit: { destructive: true },
  read: {},
  ls: {},
  grep: {},
  glob: {},
  super_read: {},
  write: {},
  edit: {},
  move: {},
  mkdir: {},
  ask_user: {},
  request_tool: {},
};

/** `extensions` 的 `kind.code` 复合键 → i18n 键（§3.1 / N16）。 */
export const EXTENSION_KIND: Readonly<Record<string, string>> = {
  "workspace.inside": "chat.tool.approval.extension.workspace.inside",
  "workspace.outside": "chat.tool.approval.extension.workspace.outside",
  "workspace.partial": "chat.tool.approval.extension.workspace.partial",
  "sandbox.enforcement.full":
    "chat.tool.approval.extension.sandbox.enforcement.full",
  "sandbox.enforcement.partial":
    "chat.tool.approval.extension.sandbox.enforcement.partial",
};

/** `intents[].action` → i18n 键（§8.2）；封闭集合外的 action 原样回显。 */
export { INTENT_ACTION } from "../toolCallDisplay";

/** `preview.verbCode` → i18n 键（§5.2：参数无判别项时的动词第二来源）。 */
export const VERB_CODE: Readonly<Record<string, string>> = {
  mkdir: "chat.tool.approval.verb.mkdir",
};

/** 参数字典可识别的判别参数；命中即视为「已知工具」（§5.2）。 */
export const DISCRIMINANT_KEYS = [
  "permanent",
  "recursive",
  "append",
  "overwrite",
] as const;

export const COMMAND_WIDTH_LIMIT = 120;
export const RESOURCE_WIDTH_LIMIT = 80;
export const CONTENT_MAX_LINES = 20;
export const CONTENT_MAX_BYTES = 1024;
export const DIFF_MAX_LINES = 40;

/** 主句优先级（§7.2）：越「专属于本用户 / 本项目」越靠前。 */
export const REASON_PRIORITY: Readonly<Record<ApprovalReason["kind"], number>> =
  {
    userRule: 0,
    projectRule: 1,
    tierCeiling: 2,
    approvalLevel: 3,
    advisor: 4,
    defaultPolicy: 5,
  };

export const EXTENSION_SEVERITY_RANK: Readonly<
  Record<ApprovalExtensionSeverity, number>
> = { danger: 0, caution: 1, info: 2 };
