import type { AdmitError, AllowedImageMimeType, Attachment } from "./types";
import {
  ALLOWED_IMAGE_MIME_TYPES,
  ATTACHMENT_MAX_FILE_SIZE_BYTES,
  ATTACHMENT_MAX_IMAGE_SIZE_BYTES,
  FILE_EXT_TO_MIME,
  IMAGE_EXT_TO_MIME,
} from "./types";
import { attachmentUrl } from "./paths";
import { storageService } from "@/services/storageService";

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

/**
 * 路径附件：先做扩展名预筛，再由 `import_attachment` 完成校验与入库。
 *
 * 入库后附件只有 `attachmentId`，前端不再保留原始绝对路径；
 * 预筛只用于快速失败，大小与类型以后端判定为准。
 */
export async function createPathAttachment(
  filePath: string,
): Promise<AttachmentResult> {
  const ext = extOf(filePath);
  const imageMime = IMAGE_EXT_TO_MIME[ext];
  const fileMime = FILE_EXT_TO_MIME[ext];
  if (!imageMime && !fileMime) return reject("invalid_file_type");

  let imported;
  try {
    imported = await storageService.importAttachment(filePath);
  } catch (e) {
    return reject(mapImportError(e));
  }

  const base = {
    id: generateAttachmentId(),
    file: new File([], imported.fileName, {
      type: imageMime ?? fileMime ?? "application/octet-stream",
    }),
    originalSizeBytes: imported.bytes,
    attachmentId: imported.attachmentId,
  };

  if (imageMime) {
    return accept({
      ...base,
      kind: "image",
      mimeType: imageMime,
      previewUrl: await attachmentUrl(imported.attachmentId).catch(() => ""),
    });
  }
  if (fileMime) {
    return accept({
      ...base,
      kind: "file",
      mimeType: fileMime,
      previewUrl: "",
    });
  }
  return reject("invalid_file_type");
}

function mapImportError(e: unknown): AdmitError {
  const message = typeof e === "string" ? e : "";
  if (message === "file_too_large") return "file_too_large";
  if (message === "too_large") return "too_large";
  if (message === "invalid_file_type") return "invalid_file_type";
  return "import_failed";
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
