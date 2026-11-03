import { open } from "@tauri-apps/plugin-dialog";
import { ALLOWED_FILE_EXTENSIONS, ALLOWED_IMAGE_EXTENSIONS } from "../types";

export async function pickAttachmentPaths(
  supportVision: boolean,
): Promise<string[]> {
  const extensions = supportVision
    ? [...new Set([...ALLOWED_IMAGE_EXTENSIONS, ...ALLOWED_FILE_EXTENSIONS])]
    : [...ALLOWED_FILE_EXTENSIONS];

  const selected = await open({
    multiple: true,
    filters: [{ name: "Images & Documents", extensions }],
  });
  if (!selected) return [];
  return Array.isArray(selected) ? selected : [selected];
}
