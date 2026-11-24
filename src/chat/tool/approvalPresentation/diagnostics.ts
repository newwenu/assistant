import type {
  ApprovalExtension,
  ApprovalPreview,
  ToolApprovalRequest,
} from "../../core/chatTypes";
import { EXTENSION_KIND, EXTENSION_SEVERITY_RANK } from "./tables";
import type { ApprovalDiagnostics, TranslateFn } from "./types";

/**
 * N7 诊断与降级：`summary` 只作诊断 / 降级原文，首屏永不渲染（§5.3）。
 *
 * `previewUnavailable` 由组合入口在同时看到 `exists` 与 `content` 后置位，
 * 这里只产出静态部分。
 */
export function buildDiagnostics(
  behavior: ToolApprovalRequest["context"]["behavior"],
  preview: ApprovalPreview | null,
  extensions: ApprovalExtension[],
  t: TranslateFn,
): ApprovalDiagnostics {
  const annotations: string[] = [];
  if (preview?.truncated) {
    annotations.push(t("chat.tool.approval.diagnostic.preview_truncated"));
  }
  if (preview?.estimated) {
    annotations.push(t("chat.tool.approval.diagnostic.preview_estimated"));
  }
  const summary = preview?.summary.trim() ?? "";
  // `summary` 为空即无正文，`truncated` / `estimated` 随之无标注可附（§3.1 / N2 / N3）。
  const previewSummary =
    summary === ""
      ? null
      : annotations.length > 0
        ? `${annotations.join(" · ")} ${summary}`
        : summary;

  const folded = extensions
    .filter(
      (extension) =>
        extension.severity !== "info" &&
        !(
          extension.kind === "workspace" &&
          (extension.code === "outside" || extension.code === "partial")
        ),
    )
    .map((extension) => {
      const key = EXTENSION_KIND[`${extension.kind}.${extension.code}`];
      return {
        kind: extension.kind,
        code: extension.code,
        severity: extension.severity,
        // 未知 `kind.code` 原样进折叠区，不本地化、不丢弃（N4 / N16）。
        label: key ? t(key) : `${extension.kind}.${extension.code}`,
      };
    })
    .sort(
      (a, b) =>
        EXTENSION_SEVERITY_RANK[a.severity] -
        EXTENSION_SEVERITY_RANK[b.severity],
    );

  return {
    readOnlyAnomaly: behavior.readOnly,
    previewSummary,
    previewUnavailable: false,
    extensions: folded,
  };
}
