import { convertFileSrc } from "@tauri-apps/api/core";
import { appDataDir, join } from "@tauri-apps/api/path";
import { openPath } from "@tauri-apps/plugin-opener";
import type { Attachment, AllowedFileMimeType } from "./attachmentTypes";
import {
  ATTACHMENT_MAX_FILES,
  ATTACHMENT_MAX_FILE_SIZE_BYTES,
  ATTACHMENT_MAX_IMAGES,
  ATTACHMENT_MAX_IMAGE_SIZE_BYTES,
  IMAGE_EXT_TO_MIME,
} from "./attachmentTypes";
import { validateImageFile, resolveImageMimeType } from "./contentBlockUtils";
import { notifyWarning, notifyInfo } from "../../stores/notifyStore";

export const FILE_EXT_TO_MIME: Record<string, AllowedFileMimeType> = {
  pdf: "application/pdf",
  txt: "text/plain",
  md: "text/markdown",
  csv: "text/csv",
  html: "text/html",
  json: "application/json",
  py: "text/x-python",
  js: "text/javascript",
  ts: "text/typescript",
  tsx: "text/typescript",
  css: "text/css",
  yaml: "text/yaml",
  yml: "text/yaml",
  xml: "text/xml",
};

export const ALLOWED_IMAGE_EXTENSIONS = Object.keys(IMAGE_EXT_TO_MIME);

function generateAttachmentId(): string {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

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

export function createAttachmentFromFile(file: File): Attachment | null {
  const validation = validateImageFile(file);
  if (!validation.ok) return null;
  const mimeType = resolveImageMimeType(file);
  if (!mimeType) return null;

  const previewUrl = URL.createObjectURL(file);
  return {
    id: generateAttachmentId(),
    kind: "image",
    file,
    previewUrl,
    mimeType,
    originalSizeBytes: file.size,
  };
}

export function createAttachmentFromPath(filePath: string): Attachment | null {
  const ext = filePath.split(".").pop()?.toLowerCase() ?? "";
  const mimeType = IMAGE_EXT_TO_MIME[ext];
  if (!mimeType) return null;

  const name = filePath.split(/[\\/]/).pop() ?? filePath;
  return {
    id: generateAttachmentId(),
    kind: "image",
    file: new File([], name, { type: mimeType }),
    previewUrl: convertFileSrc(filePath),
    mimeType,
    originalSizeBytes: 0,
    filePath,
  };
}

export function createFileAttachmentFromFile(file: File): Attachment | null {
  if (file.size > ATTACHMENT_MAX_FILE_SIZE_BYTES) return null;

  const ext = file.name.split(".").pop()?.toLowerCase() ?? "";
  const mimeType = FILE_EXT_TO_MIME[ext];
  if (!mimeType) return null;

  return {
    id: generateAttachmentId(),
    kind: "file",
    file,
    previewUrl: "",
    mimeType,
    originalSizeBytes: file.size,
  };
}

export function createFileAttachmentFromPath(
  filePath: string,
): Attachment | null {
  const ext = filePath.split(".").pop()?.toLowerCase() ?? "";
  const mimeType = FILE_EXT_TO_MIME[ext];
  if (!mimeType) return null;

  const name = filePath.split(/[\\/]/).pop() ?? filePath;
  return {
    id: generateAttachmentId(),
    kind: "file",
    file: new File([], name, { type: mimeType }),
    previewUrl: "",
    mimeType,
    originalSizeBytes: 0,
    filePath,
  };
}

export type AttachValidationError =
  | "too_many"
  | "too_large"
  | "invalid_type"
  | "no_vision"
  | "too_many_files"
  | "file_too_large"
  | "invalid_file_type";

export function notifyAttachError(error: AttachValidationError): void {
  switch (error) {
    case "too_many":
      notifyWarning(
        "chat.input.too_many_images",
        "chat.input.too_many_images",
        { max: ATTACHMENT_MAX_IMAGES },
      );
      break;
    case "too_large":
      notifyWarning(
        "chat.input.image_too_large",
        "chat.input.image_too_large",
        { max: ATTACHMENT_MAX_IMAGE_SIZE_BYTES / (1024 * 1024) },
      );
      break;
    case "invalid_type":
      notifyWarning(
        "chat.input.invalid_image_type",
        "chat.input.invalid_image_type",
      );
      break;
    case "no_vision":
      notifyInfo("chat.input.model_no_vision", "chat.input.model_no_vision");
      break;
    case "too_many_files":
      notifyWarning("chat.input.too_many_files", "chat.input.too_many_files", {
        max: ATTACHMENT_MAX_FILES,
      });
      break;
    case "file_too_large":
      notifyWarning("chat.input.file_too_large", "chat.input.file_too_large", {
        max: ATTACHMENT_MAX_FILE_SIZE_BYTES / (1024 * 1024),
      });
      break;
    case "invalid_file_type":
      notifyWarning(
        "chat.input.invalid_file_type",
        "chat.input.invalid_file_type",
      );
      break;
  }
}
