export const ALLOWED_IMAGE_MIME_TYPES = [
  "image/png",
  "image/jpeg",
  "image/webp",
  "image/gif",
  "image/bmp",
  "image/tiff",
] as const;

export type AllowedImageMimeType = (typeof ALLOWED_IMAGE_MIME_TYPES)[number];

export const IMAGE_EXT_TO_MIME: Record<string, AllowedImageMimeType> = {
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  jfif: "image/jpeg",
  gif: "image/gif",
  webp: "image/webp",
  bmp: "image/bmp",
  tif: "image/tiff",
  tiff: "image/tiff",
};

export const ALLOWED_IMAGE_EXTENSIONS = Object.keys(IMAGE_EXT_TO_MIME);

export const ALLOWED_FILE_MIME_TYPES = [
  "application/pdf",
  "text/plain",
  "text/markdown",
  "text/csv",
  "text/html",
  "application/json",
  "text/x-python",
  "text/javascript",
  "text/typescript",
  "text/css",
  "text/yaml",
  "text/xml",
] as const;

export type AllowedFileMimeType = (typeof ALLOWED_FILE_MIME_TYPES)[number];

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

export const ALLOWED_FILE_EXTENSIONS = [
  "pdf",
  "txt",
  "md",
  "csv",
  "html",
  "json",
  "py",
  "js",
  "ts",
  "tsx",
  "css",
  "yaml",
  "yml",
  "xml",
] as const;

export type AttachmentMimeType = AllowedImageMimeType | AllowedFileMimeType;

export type AttachmentKind = "image" | "file";

interface BaseAttachment {
  id: string;
  file: File;
  previewUrl: string;
  originalSizeBytes: number;
  width?: number;
  height?: number;
  attachmentId?: string;
}

export interface ImageAttachment extends BaseAttachment {
  kind: "image";
  mimeType: AllowedImageMimeType;
}

export interface FileAttachment extends BaseAttachment {
  kind: "file";
  mimeType: AllowedFileMimeType;
}

export type Attachment = ImageAttachment | FileAttachment;

export const ATTACHMENT_MAX_IMAGES = 4;
export const ATTACHMENT_MAX_FILES = 4;
export const ATTACHMENT_MAX_IMAGE_SIZE_BYTES = 8 * 1024 * 1024;
export const ATTACHMENT_MAX_FILE_SIZE_BYTES = 16 * 1024 * 1024;

export type AdmitError =
  | "too_many"
  | "too_large"
  | "invalid_type"
  | "no_vision"
  | "too_many_files"
  | "file_too_large"
  | "invalid_file_type"
  | "import_failed";
