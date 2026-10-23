import type {
  ContentBlock,
  TextBlock,
  ImageBlock,
  ImageAttachmentRef,
} from "./contentBlockTypes";
import type { Attachment } from "./attachmentTypes";
import {
  ALLOWED_IMAGE_MIME_TYPES,
  ATTACHMENT_MAX_IMAGE_SIZE_BYTES,
} from "./attachmentTypes";

export function buildContentBlocks(
  text: string,
  attachments: Attachment[],
): ContentBlock[] {
  const blocks: ContentBlock[] = [];

  if (text.trim().length > 0) {
    blocks.push({ type: "text", text: text.trim() } satisfies TextBlock);
  }

  for (const att of attachments) {
    if (att.kind === "image") {
      const ref: ImageAttachmentRef = {
        attachmentId: att.attachmentId ?? "",
        mediaType: att.mimeType,
        bytes: att.originalSizeBytes,
        width: att.width,
        height: att.height,
        name: att.file.name,
      };
      blocks.push({ type: "image", attachment: ref } satisfies ImageBlock);
    }
  }

  return blocks;
}

export function parseContent(content: string): ContentBlock[] | null {
  try {
    const parsed: unknown = JSON.parse(content);
    if (
      Array.isArray(parsed) &&
      parsed.length > 0 &&
      parsed.every(
        (b): b is Record<string, unknown> =>
          typeof b === "object" &&
          b !== null &&
          "type" in b &&
          typeof (b as Record<string, unknown>).type === "string",
      )
    ) {
      // @ts-expect-error — safe: every() above verified each element has type:string; see spec §3.3
      return parsed as ContentBlock[];
    }
    return null;
  } catch {
    return null;
  }
}

export function serializeContent(blocks: ContentBlock[]): string {
  return JSON.stringify(blocks);
}

export function validateImageFile(file: File): { ok: boolean; error?: string } {
  if (
    !ALLOWED_IMAGE_MIME_TYPES.includes(
      file.type as (typeof ALLOWED_IMAGE_MIME_TYPES)[number],
    )
  ) {
    return { ok: false, error: "invalid_image_type" };
  }
  if (file.size > ATTACHMENT_MAX_IMAGE_SIZE_BYTES) {
    return { ok: false, error: "image_too_large" };
  }
  return { ok: true };
}

export function extractPlainText(content: string): string {
  const blocks = parseContent(content);
  if (blocks) {
    return blocks
      .filter((b): b is TextBlock => b.type === "text")
      .map((b) => b.text)
      .join("\n");
  }
  return content;
}

export function rebuildContent(
  originalContent: string,
  newText: string,
): string {
  const blocks = parseContent(originalContent);
  if (blocks) {
    const rebuilt: ContentBlock[] = [
      { type: "text", text: newText },
      ...blocks.filter((b) => b.type === "image"),
    ];
    return serializeContent(rebuilt);
  }
  return newText;
}
