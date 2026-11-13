import { open } from "@tauri-apps/plugin-dialog";
import type { VisionSupport } from "../types";
import { ALLOWED_FILE_EXTENSIONS, ALLOWED_IMAGE_EXTENSIONS } from "../types";

export async function pickAttachmentPaths(
  visionSupport: VisionSupport,
): Promise<string[]> {
  // 只有「确定不支持」才隐藏图片扩展名；「不知道」保留入口，由准入放行并标注
  const extensions =
    visionSupport === "unsupported"
      ? [...ALLOWED_FILE_EXTENSIONS]
      : [...new Set([...ALLOWED_IMAGE_EXTENSIONS, ...ALLOWED_FILE_EXTENSIONS])];

  const selected = await open({
    multiple: true,
    filters: [{ name: "Images & Documents", extensions }],
  });
  if (!selected) return [];
  return Array.isArray(selected) ? selected : [selected];
}
