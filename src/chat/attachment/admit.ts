import type { AdmitError, Attachment, VisionSupport } from "./types";
import { attachmentLimits } from "./limits";

export interface AdmitContext {
  attachments: readonly Attachment[];
  visionSupport: VisionSupport;
}

export function admitAttachment(
  att: Attachment,
  ctx: AdmitContext,
): AdmitError | null {
  const limits = attachmentLimits();
  const sameKindCount = ctx.attachments.filter(
    (a) => a.kind === att.kind,
  ).length;

  if (att.kind === "image") {
    // 只有「确定不支持」才拦；「不知道」放行（附一条标注），否则三态退化成布尔
    if (ctx.visionSupport === "unsupported") return "no_vision";
    if (sameKindCount >= limits.maxImages) return "too_many";
    return null;
  }

  if (sameKindCount >= limits.maxFiles) return "too_many_files";
  return null;
}
