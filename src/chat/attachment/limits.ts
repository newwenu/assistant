import type { AttachmentLimitsDto } from "@/types/protocol.contract";
import { storageService } from "@/services/desktop/storageService";

let cached: AttachmentLimitsDto | null = null;
let limitsPromise: Promise<AttachmentLimitsDto> | null = null;

/**
 * 限额只来自后端 `ATTACHMENT_LIMITS`，前端不持有任何数值字面量。
 * 未加载时同步读会抛错——调用方必须保证在用户可点击附件入口前完成预取。
 */
export function attachmentLimits(): AttachmentLimitsDto {
  if (!cached) {
    throw new Error("attachment limits are not loaded yet");
  }
  return cached;
}

export function prefetchAttachmentLimits(): Promise<AttachmentLimitsDto> {
  if (!limitsPromise) {
    limitsPromise = storageService
      .getAttachmentLimits()
      .then((limits) => {
        cached = limits;
        return limits;
      })
      .catch((e: unknown) => {
        limitsPromise = null;
        throw e;
      });
  }
  return limitsPromise;
}

/** 测试专用：清空缓存，避免用例间互相污染。 */
export function resetAttachmentLimitsForTest(): void {
  cached = null;
  limitsPromise = null;
}
