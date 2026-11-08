import type { AdmitError, Attachment } from "./types";
import { ATTACHMENT_MAX_FILES, ATTACHMENT_MAX_IMAGES } from "./types";

export interface AdmitContext {
  attachments: readonly Attachment[];
  supportVision: boolean;
}

export function admitAttachment(
  att: Attachment,
  ctx: AdmitContext,
): AdmitError | null {
  const sameKindCount = ctx.attachments.filter(
    (a) => a.kind === att.kind,
  ).length;

  if (att.kind === "image") {
    if (!ctx.supportVision) return "no_vision";
    if (sameKindCount >= ATTACHMENT_MAX_IMAGES) return "too_many";
    return null;
  }

  if (sameKindCount >= ATTACHMENT_MAX_FILES) return "too_many_files";
  return null;
}
