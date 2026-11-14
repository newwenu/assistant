import type {
  ApprovalExtension,
  ApprovalExtensionSeverity,
  ToolCallView,
} from "../core/chatTypes";
import type { ToolResultCompactDto } from "../../types/protocol.contract";

const ARG_SUMMARY_MAX_CHARS = 240;
const RESULT_COLLAPSE_LINES = 12;
const RESULT_COLLAPSE_BYTES = 1024;

/** 参数摘要优先取这些字段，覆盖 shell / 读写 / 检索类工具的主要参数。 */
const PRIMARY_ARG_KEYS = [
  "command",
  "cmd",
  "path",
  "filePath",
  "file_path",
  "pattern",
  "url",
  "query",
  "name",
] as const;

export interface PersistedOutput {
  path: string;
  preview: string;
}

export interface ResultStats {
  lines: number;
  bytes: number;
}

/** 扩展区固定排序：`danger` → `caution` → `info`，同级保持后端顺序（I38d §3.2.2）。 */
const SEVERITY_RANK: Record<ApprovalExtensionSeverity, number> = {
  danger: 0,
  caution: 1,
  info: 2,
};

export function sortApprovalExtensions(
  extensions: ApprovalExtension[],
): ApprovalExtension[] {
  return [...extensions].sort(
    (a, b) => SEVERITY_RANK[a.severity] - SEVERITY_RANK[b.severity],
  );
}

function clip(text: string, max: number): string {
  return text.length <= max ? text : `${text.slice(0, max)}…`;
}

function argText(value: unknown): string | null {
  if (typeof value === "string") {
    const trimmed = value.trim();
    return trimmed ? trimmed : null;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  if (Array.isArray(value)) {
    const parts = value
      .map(argText)
      .filter((part): part is string => part !== null);
    return parts.length > 0 ? parts.join(" ") : null;
  }
  return null;
}

/**
 * 单行参数摘要：按固定优先级挑一个可读字段（`command` 数组按 argv 拼接）。
 * 参数缺失或全为非标量时返回 `null`，由调用方隐藏该行。
 */
export function summarizeArguments(
  args?: Record<string, unknown>,
): string | null {
  if (!args) return null;
  for (const key of PRIMARY_ARG_KEYS) {
    const text = argText(args[key]);
    if (text) return clip(text, ARG_SUMMARY_MAX_CHARS);
  }
  for (const value of Object.values(args)) {
    const text = argText(value);
    if (text) return clip(text, ARG_SUMMARY_MAX_CHARS);
  }
  return null;
}

/**
 * 解析后端落盘回填文本 `<persisted-output path="...">\n<preview>`。
 * 非该形状返回 `null`，调用方按普通结果渲染。
 */
export function parsePersistedOutput(
  text?: string | null,
): PersistedOutput | null {
  if (!text) return null;
  const match = /^<persisted-output path="([^"]+)">\r?\n?([\s\S]*)$/.exec(text);
  if (!match) return null;
  return { path: match[1], preview: match[2] };
}

export function resultStats(text: string): ResultStats {
  if (!text) return { lines: 0, bytes: 0 };
  return {
    lines: text.split("\n").length,
    bytes: new TextEncoder().encode(text).length,
  };
}

/** 大结果默认折叠；阈值只影响默认展开态，不限制渲染内容。 */
export function shouldCollapseResult(text: string): boolean {
  const stats = resultStats(text);
  return (
    stats.lines > RESULT_COLLAPSE_LINES || stats.bytes > RESULT_COLLAPSE_BYTES
  );
}

export function formatToolDuration(
  startedAt?: number | null,
  finishedAt?: number | null,
): string | null {
  if (startedAt == null || finishedAt == null) return null;
  const ms = Math.max(0, finishedAt - startedAt);
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  const minutes = Math.floor(ms / 60_000);
  const seconds = Math.round((ms % 60_000) / 1000);
  return `${minutes}m${seconds}s`;
}

export interface ResultDisplayPersisted {
  path: string;
  bytes: number | null;
}

/**
 * 结果区的展示投影：`compact` 存在时一切取自信封，不再解析正文文本；
 * `compact == null`（旧数据）才回落到 `parsePersistedOutput` / `resultStats`。
 */
export interface ResultDisplay {
  /** 单行标题：信封 `head`；文本兜底时为空。 */
  head: string | null;
  /** 展开后原样显示的正文（不改字节）。 */
  body: string;
  /** 默认是否折叠：信封已概括，正文默认收起；文本兜底按既有阈值。 */
  defaultCollapsed: boolean;
  persisted: ResultDisplayPersisted | null;
  truncated: ToolResultCompactDto["truncated"];
  errorKind: ToolResultCompactDto["errorKind"];
  source: "compact" | "text";
}

/**
 * 去掉正文里由后端渲染的落盘标签前缀。路径来自 `compact.persisted`，
 * 因此这里只是按已知前缀裁剪，不做正则反解（门禁 7：定位符非解析）。
 */
function trimPersistedTag(body: string, path: string): string {
  const tag = `<persisted-output path="${path}">`;
  if (!body.startsWith(tag)) return body;
  return body.slice(tag.length).replace(/^\r?\n/, "");
}

export function resolveResultDisplay(
  resultText: string | null | undefined,
  compact: ToolResultCompactDto | null | undefined,
): ResultDisplay {
  const text = resultText ?? "";
  if (compact) {
    return {
      head: compact.head,
      body: compact.persisted
        ? trimPersistedTag(text, compact.persisted.path)
        : text,
      defaultCollapsed: true,
      persisted: compact.persisted
        ? { path: compact.persisted.path, bytes: compact.persisted.bytes }
        : null,
      truncated: compact.truncated,
      errorKind: compact.errorKind,
      source: "compact",
    };
  }
  const persisted = parsePersistedOutput(text);
  const body = persisted ? persisted.preview : text;
  return {
    head: null,
    body,
    defaultCollapsed: shouldCollapseResult(body),
    persisted: persisted ? { path: persisted.path, bytes: null } : null,
    truncated: null,
    errorKind: null,
    source: "text",
  };
}

/**
 * 渲染选路判据（I46 compact §3.7.4）：只有「锚点存在」且「面板正打开该文件」
 * 才走文档批注引用。**不读** `role` / `modeId` / `TurnKind` / 工具名（门禁 13）。
 */
export function shouldRenderDocStub(
  view: Pick<ToolCallView, "compact">,
  docPanelPath: string | null | undefined,
): boolean {
  const anchor = view.compact?.anchor ?? null;
  return anchor != null && docPanelPath != null && anchor.path === docPanelPath;
}
