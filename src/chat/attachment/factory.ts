import { convertFileSrc } from "@tauri-apps/api/core";
import type { AdmitError, AllowedImageMimeType, Attachment } from "./types";
import {
  ALLOWED_IMAGE_MIME_TYPES,
  ATTACHMENT_MAX_FILE_SIZE_BYTES,
  ATTACHMENT_MAX_IMAGE_SIZE_BYTES,
  FILE_EXT_TO_MIME,
  IMAGE_EXT_TO_MIME,
} from "./types";

export type AttachmentResult =
  { ok: true; attachment: Attachment } | { ok: false; error: AdmitError };

function reject(error: AdmitError): AttachmentResult {
  return { ok: false, error };
}

function accept(attachment: Attachment): AttachmentResult {
  return { ok: true, attachment };
}

function generateAttachmentId(): string {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

function extOf(name: string): string {
  return name.split(".").pop()?.toLowerCase() ?? "";
}

function fileNameFromPath(filePath: string): string {
  return filePath.split(/[\\/]/).pop() ?? filePath;
}

export function resolveImageMimeType(file: File): AllowedImageMimeType | null {
  if (
    ALLOWED_IMAGE_MIME_TYPES.includes(
      file.type as (typeof ALLOWED_IMAGE_MIME_TYPES)[number],
    )
  ) {
    return file.type as AllowedImageMimeType;
  }
  // 部分平台对 bmp/tiff/jfif 不返回 MIME，回退到扩展名判断
  return IMAGE_EXT_TO_MIME[extOf(file.name)] ?? null;
}

export function validateImageFile(file: File): AdmitError | null {
  if (!resolveImageMimeType(file)) return "invalid_type";
  if (file.size > ATTACHMENT_MAX_IMAGE_SIZE_BYTES) return "too_large";
  return null;
}

export function createImageAttachmentFromFile(file: File): AttachmentResult {
  const error = validateImageFile(file);
  if (error) return reject(error);
  const mimeType = resolveImageMimeType(file);
  if (!mimeType) return reject("invalid_type");

  return accept({
    id: generateAttachmentId(),
    kind: "image",
    file,
    previewUrl: URL.createObjectURL(file),
    mimeType,
    originalSizeBytes: file.size,
  });
}

export function createImageAttachmentFromPath(
  filePath: string,
): AttachmentResult {
  const mimeType = IMAGE_EXT_TO_MIME[extOf(filePath)];
  if (!mimeType) return reject("invalid_type");

  return accept({
    id: generateAttachmentId(),
    kind: "image",
    file: new File([], fileNameFromPath(filePath), { type: mimeType }),
    previewUrl: convertFileSrc(filePath),
    mimeType,
    originalSizeBytes: 0,
    filePath,
  });
}

export function createFileAttachmentFromFile(file: File): AttachmentResult {
  const mimeType = FILE_EXT_TO_MIME[extOf(file.name)];
  if (!mimeType) return reject("invalid_file_type");
  if (file.size > ATTACHMENT_MAX_FILE_SIZE_BYTES)
    return reject("file_too_large");

  return accept({
    id: generateAttachmentId(),
    kind: "file",
    file,
    previewUrl: "",
    mimeType,
    originalSizeBytes: file.size,
  });
}

export function createFileAttachmentFromPath(
  filePath: string,
): AttachmentResult {
  const mimeType = FILE_EXT_TO_MIME[extOf(filePath)];
  if (!mimeType) return reject("invalid_file_type");

  return accept({
    id: generateAttachmentId(),
    kind: "file",
    file: new File([], fileNameFromPath(filePath), { type: mimeType }),
    previewUrl: "",
    mimeType,
    originalSizeBytes: 0,
    filePath,
  });
}
