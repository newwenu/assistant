export const ALLOWED_IMAGE_MIME_TYPES = [
  "image/png",
  "image/jpeg",
  "image/webp",
  "image/gif",
] as const;

export type AllowedImageMimeType = (typeof ALLOWED_IMAGE_MIME_TYPES)[number];

export type AttachmentKind = "image" | "file";

export interface Attachment {
  id: string;
  kind: AttachmentKind;
  file: File;
  previewUrl: string;
  mimeType: AllowedImageMimeType;
  originalSizeBytes: number;
  width?: number;
  height?: number;
  attachmentId?: string;
  filePath?: string;
}

export const ATTACHMENT_MAX_IMAGES = 4;
export const ATTACHMENT_MAX_FILES = 4;
export const ATTACHMENT_MAX_IMAGE_SIZE_BYTES = 8 * 1024 * 1024;
export const ATTACHMENT_MAX_FILE_SIZE_BYTES = 16 * 1024 * 1024;
