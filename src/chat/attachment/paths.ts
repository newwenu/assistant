import { appDataDir, join } from "@tauri-apps/api/path";
import { storageService } from "@/services/storageService";

let attachmentsDirCache: string | null = null;

export async function getAttachmentsDir(): Promise<string> {
  if (!attachmentsDirCache) {
    attachmentsDirCache = await join(await appDataDir(), "attachments");
  }
  return attachmentsDirCache;
}

/**
 * 由后端完成校验、存在性判定与打开。
 * 返回 `false` 表示附件文件不存在（或 id 非法），调用方据此提示而不依赖 shell 报错。
 */
export async function openAttachmentFile(
  attachmentId: string,
): Promise<boolean> {
  if (!attachmentId) return false;
  return storageService.openAttachment(attachmentId);
}
