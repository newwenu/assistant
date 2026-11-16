import type {
  ContentBlock,
  TextBlock,
  ImageBlock,
  FileBlock,
  ImageAttachmentRef,
  FileAttachmentRef,
} from "./contentBlockTypes";
import type { Attachment } from "../../attachment/types";

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
    } else if (att.kind === "file") {
      const ref: FileAttachmentRef = {
        attachmentId: att.attachmentId ?? "",
        name: att.file.name,
        bytes: att.originalSizeBytes,
      };
      blocks.push({ type: "file", attachment: ref } satisfies FileBlock);
    }
  }

  return blocks;
}

export async function buildContentBlocksAsync(
  text: string,
  attachments: Attachment[],
): Promise<ContentBlock[]> {
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
      if (ref.attachmentId) {
        blocks.push({ type: "image", attachment: ref } satisfies ImageBlock);
      } else {
        const cached_data = await fileToBase64(att.file);
        blocks.push({
          type: "image",
          attachment: ref,
          cached_data,
        } satisfies ImageBlock);
      }
    } else if (att.kind === "file") {
      const ref: FileAttachmentRef = {
        attachmentId: att.attachmentId ?? "",
        name: att.file.name,
        bytes: att.originalSizeBytes,
      };
      if (ref.attachmentId) {
        blocks.push({ type: "file", attachment: ref } satisfies FileBlock);
      } else {
        const cached_data = await fileToBase64(att.file);
        blocks.push({
          type: "file",
          attachment: ref,
          cached_data,
        } satisfies FileBlock);
      }
    }
  }

  return blocks;
}

function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const dataUrl = reader.result as string;
      const base64 = dataUrl.split(",")[1] ?? "";
      resolve(base64);
    };
    reader.onerror = () => {
      reject(reader.error ?? new Error("Failed to read file as base64"));
    };
    reader.readAsDataURL(file);
  });
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

function derivedTextOfBlock(block: ContentBlock): string | null {
  // 向前兼容：parseContent 允许未知 type 通过，故按宽类型读取而非依赖封闭联合
  const b: { type: string; text?: unknown; attachment?: { name?: string } } =
    block;
  switch (b.type) {
    case "text":
      return typeof b.text === "string" ? b.text : null;
    case "image":
      return "[图片]";
    case "file":
      return b.attachment?.name ? `[文件 ${b.attachment.name}]` : "[文件]";
    default:
      return `[${b.type}]`;
  }
}

export function extractDerivedText(content: string): string {
  const blocks = parseContent(content);
  if (!blocks) return content;
  return blocks
    .map(derivedTextOfBlock)
    .filter((t): t is string => t !== null)
    .join("\n");
}

export function rebuildContent(
  originalContent: string,
  newText: string,
): string {
  const blocks = parseContent(originalContent);
  if (blocks) {
    const rebuilt: ContentBlock[] = [
      { type: "text", text: newText },
      ...blocks.filter((b) => b.type === "image" || b.type === "file"),
    ];
    return serializeContent(rebuilt);
  }
  return newText;
}
