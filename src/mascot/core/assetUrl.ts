import { assetDirs, assetUrl } from "@/services/desktop/assetDirs";

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
 * `src` 是否指向本地素材文件（`convertFileSrc` 产物），而非远端 URL。
 * 伴生声明只属于安装进本地素材目录的那份文件，手填的远端 URL 不能套用本地声明。
 * Windows 为 `http://asset.localhost/<encoded>`，其它平台为 `asset://localhost/<encoded>`。
 */
export function isLocalAssetUrl(src: string): boolean {
  const lower = src.toLowerCase();
  return (
    lower.startsWith("http://asset.localhost/") ||
    lower.startsWith("https://asset.localhost/") ||
    lower.startsWith("asset://")
  );
}
