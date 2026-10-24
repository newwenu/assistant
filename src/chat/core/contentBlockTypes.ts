export interface TextBlock {
  type: "text";
  text: string;
}

export interface ImageBlock {
  type: "image";
  attachment: ImageAttachmentRef;
  cached_data?: string;
  file_path?: string;
  offloaded?: true;
}

export interface FileBlock {
  type: "file";
  attachment: FileAttachmentRef;
}

export interface ImageAttachmentRef {
  attachmentId: string;
  mediaType: ImageMediaType;
  bytes: number;
  width?: number;
  height?: number;
  name?: string;
}

export interface FileAttachmentRef {
  attachmentId: string;
  name: string;
  bytes: number;
}

export type ImageMediaType =
  "image/png" | "image/jpeg" | "image/webp" | "image/gif";

export type ContentBlock = TextBlock | ImageBlock | FileBlock;
