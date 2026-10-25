import { convertFileSrc } from "@tauri-apps/api/core";
import type { Attachment, AllowedImageMimeType } from "./attachmentTypes";
import {
  ATTACHMENT_MAX_IMAGES,
  ATTACHMENT_MAX_IMAGE_SIZE_BYTES,
} from "./attachmentTypes";
import { validateImageFile } from "./contentBlockUtils";
import { notifyWarning, notifyInfo } from "../../stores/notifyStore";

export const EXT_TO_MIME: Record<string, AllowedImageMimeType> = {
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  gif: "image/gif",
  webp: "image/webp",
};

export const ALLOWED_IMAGE_EXTENSIONS = Object.keys(EXT_TO_MIME);

function generateAttachmentId(): string {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

export function createAttachmentFromFile(file: File): Attachment | null {
  const validation = validateImageFile(file);
  if (!validation.ok) return null;

  const previewUrl = URL.createObjectURL(file);
  return {
    id: generateAttachmentId(),
    kind: "image",
    file,
    previewUrl,
    mimeType: file.type as AllowedImageMimeType,
    originalSizeBytes: file.size,
  };
}

export function createAttachmentFromPath(filePath: string): Attachment | null {
  const ext = filePath.split(".").pop()?.toLowerCase() ?? "";
  const mimeType = EXT_TO_MIME[ext];
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

export type AttachValidationError =
  "too_many" | "too_large" | "invalid_type" | "no_vision";

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
  }
}
