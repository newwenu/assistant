import { convertFileSrc } from "@tauri-apps/api/core";
import { join } from "@tauri-apps/api/path";
import type { AssetDirsDto } from "@/types/protocol.contract";
import { storageService } from "@/services/desktop/storageService";

let assetDirsPromise: Promise<AssetDirsDto> | null = null;

async function assetDirs(): Promise<AssetDirsDto> {
  if (!assetDirsPromise) {
    assetDirsPromise = storageService.getAssetDirs().catch((e: unknown) => {
      assetDirsPromise = null;
      throw e;
    });
  }
  return assetDirsPromise;
}

async function assetUrl(baseDir: string, name: string): Promise<string> {
  return convertFileSrc(await join(baseDir, name));
}

/**
 * 附件文件在 asset 作用域内的 URL。`id` 由 `import_attachment` 返回，
 * 前端只拼一次目录，不再持有目录名与绝对路径知识。
 */
export async function attachmentUrl(id: string): Promise<string> {
  const dirs = await assetDirs();
  return assetUrl(dirs.attachmentsDir, id);
}

/**
 * 桌宠素材 URL。`relPath` 由 `install_mascot_asset` 返回（相对素材根），
 * `version` 用于刷新缓存。
 */
export async function mascotAssetUrl(
  relPath: string,
  version?: number,
): Promise<string> {
  const dirs = await assetDirs();
  const url = await assetUrl(dirs.mascotAssetsDir, relPath);
  return version === undefined ? url : `${url}?v=${version}`;
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
