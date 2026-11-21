import { assetDirs, assetUrl } from "@/services/desktop/assetDirs";
import { storageService } from "@/services/desktop/storageService";

/**
 * 附件文件在 asset 作用域内的 URL。`id` 由 `import_attachment` 返回，
 * 前端只拼一次目录，不再持有目录名与绝对路径知识。
 */
export async function attachmentUrl(id: string): Promise<string> {
  const dirs = await assetDirs();
  return assetUrl(dirs.attachmentsDir, id);
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
