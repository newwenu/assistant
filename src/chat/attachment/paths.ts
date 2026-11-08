import { appDataDir, join } from "@tauri-apps/api/path";
import { openPath } from "@tauri-apps/plugin-opener";

let attachmentsDirCache: string | null = null;

export async function getAttachmentsDir(): Promise<string> {
  if (!attachmentsDirCache) {
    attachmentsDirCache = await join(await appDataDir(), "attachments");
  }
  return attachmentsDirCache;
}

/** 用系统默认程序打开已入库的附件；附件 id 为空（尚未发送）时不做处理。 */
export async function openAttachmentFile(attachmentId: string): Promise<void> {
  if (!attachmentId) return;
  const dir = await getAttachmentsDir();
  await openPath(await join(dir, attachmentId));
}
