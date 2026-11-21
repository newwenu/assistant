/**
 * `detail` 文法解析（I50 §2.4 / §6）。
 *
 * 规则与后端唯一解析入口 `assistant-vault::audit::parse_detail` 逐条对齐：
 * 段数 + 首段封闭标签有序匹配，含 `@` 形态按**首个 `@`** 切分，其余原样兜底。
 * 本模块只做展示分类，**不**从中推断条目身份：条目 ID 一律取 DTO 的 `entryId`。
 */

export const DETAIL_CLOSED_TAGS = ["totp", "conflict", "json", "csv"] as const;
export const DETAIL_EXPORT_FORMATS = ["json", "csv"] as const;

export type ParsedAuditDetail =
  | { kind: "empty" }
  | { kind: "count"; count: number }
  | { kind: "entryId"; entryId: string }
  | { kind: "entryWithSuffix"; entryId: string | null; suffix: string }
  | { kind: "taggedEntry"; tag: string; entryId: string | null }
  | { kind: "entryCount"; entryId: string | null; count: number }
  | {
      kind: "export";
      format: string;
      count: number;
      digest: string;
      failed: boolean;
    }
  | {
      kind: "dekRotated";
      oldVersion: number;
      newVersion: number;
      count: number;
    }
  | {
      kind: "conflict";
      entryId: string | null;
      side: string;
      field: string;
      digest: string;
    }
  | { kind: "fingerprint"; keyType: string; digest: string }
  | { kind: "label"; label: string }
  | { kind: "raw"; raw: string };

const UNKNOWN_SEGMENT = "unknown";

/**
 * 条目 ID 由 `uuid::Uuid::new_v4()` 生成，落库形态为带连字符的 32 位十六进制；
 * 同时接受后端 `Uuid::parse_str` 也认可的紧凑形态，避免两侧判定分叉。
 */
const UUID_PATTERN =
  /^(?:[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}|[0-9a-fA-F]{32})$/;

function isClosedTag(segment: string): boolean {
  return (DETAIL_CLOSED_TAGS as readonly string[]).includes(segment);
}

function isExportFormat(segment: string): boolean {
  return (DETAIL_EXPORT_FORMATS as readonly string[]).includes(segment);
}

function parseCount(segment: string): number | null {
  if (segment.length === 0 || !/^[0-9]+$/.test(segment)) return null;
  const value = Number(segment);
  return Number.isSafeInteger(value) ? value : null;
}

function uuidOrNull(segment: string): string | null {
  return UUID_PATTERN.test(segment) ? segment : null;
}

function isBase64Digest(segment: string): boolean {
  return segment.length > 0 && /^[A-Za-z0-9+/=]+$/.test(segment);
}

export function parseAuditDetail(
  detail: string | null | undefined,
): ParsedAuditDetail {
  if (detail === null || detail === undefined || detail.length === 0) {
    return { kind: "empty" };
  }

  const segments = detail.split(":");
  const first = segments[0];

  if (isClosedTag(first)) {
    if (first === "conflict") {
      if (segments.length !== 5) return { kind: "raw", raw: detail };
      return {
        kind: "conflict",
        entryId: uuidOrNull(segments[1]),
        side: segments[2],
        field: segments[3],
        digest: segments[4],
      };
    }

    if (isExportFormat(first)) {
      if (segments.length !== 3 && segments.length !== 4) {
        return { kind: "raw", raw: detail };
      }
      const count = parseCount(segments[1]);
      if (count === null) return { kind: "raw", raw: detail };
      const failed = segments.length === 4;
      if (failed && segments[3] !== "failed") {
        return { kind: "raw", raw: detail };
      }
      return {
        kind: "export",
        format: first,
        count,
        digest: segments[2],
        failed,
      };
    }

    if (segments.length !== 2) return { kind: "raw", raw: detail };
    return {
      kind: "taggedEntry",
      tag: first,
      entryId: uuidOrNull(segments[1]),
    };
  }

  if (first === "SHA256") {
    if (segments.length !== 2 || !isBase64Digest(segments[1])) {
      return { kind: "raw", raw: detail };
    }
    return { kind: "fingerprint", keyType: first, digest: segments[1] };
  }

  const at = detail.indexOf("@");
  if (at >= 0) {
    const head = detail.slice(0, at);
    const suffix = detail.slice(at + 1);
    return {
      kind: "entryWithSuffix",
      entryId: uuidOrNull(head),
      suffix,
    };
  }

  if (segments.length === 2) {
    const count = parseCount(segments[1]);
    if (count !== null) {
      return { kind: "entryCount", entryId: uuidOrNull(first), count };
    }
  }

  if (segments.length === 3) {
    const oldVersion = parseCount(segments[0]);
    const newVersion = parseCount(segments[1]);
    const count = parseCount(segments[2]);
    if (oldVersion !== null && newVersion !== null && count !== null) {
      return { kind: "dekRotated", oldVersion, newVersion, count };
    }
  }

  if (segments.length === 1) {
    const count = parseCount(first);
    if (count !== null) return { kind: "count", count };
    const entryId = uuidOrNull(first);
    if (entryId !== null) return { kind: "entryId", entryId };
    return { kind: "label", label: first };
  }

  return { kind: "raw", raw: detail };
}

/**
 * `detail` 的 ID 段是否为封闭哨兵值 `unknown`（§2.4「入参校验前置」）。
 * 哨兵不指向任何真实条目，展示时必须按未知值渲染且不可跳转。
 */
export function isUnknownSegment(segment: string): boolean {
  return segment === UNKNOWN_SEGMENT;
}
