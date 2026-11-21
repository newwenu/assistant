import { convertFileSrc } from "@tauri-apps/api/core";
import { join } from "@tauri-apps/api/path";
import type { AssetDirsDto } from "@/types/protocol.contract";
import { storageService } from "@/services/desktop/storageService";

let assetDirsPromise: Promise<AssetDirsDto> | null = null;

export async function assetDirs(): Promise<AssetDirsDto> {
  if (!assetDirsPromise) {
    assetDirsPromise = storageService.getAssetDirs().catch((e: unknown) => {
      assetDirsPromise = null;
      throw e;
    });
  }
  return assetDirsPromise;
}

/**
 * 目录 + 文件名到 asset 作用域 URL 的唯一拼接点。
 * 各域只传后端给出的目录与文件名，不自行 `join` / `convertFileSrc`。
 */
export async function assetUrl(baseDir: string, name: string): Promise<string> {
  return convertFileSrc(await join(baseDir, name));
}
