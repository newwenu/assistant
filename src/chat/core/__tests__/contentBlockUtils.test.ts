import { describe, it, expect } from "vitest";
import {
  buildContentBlocks,
  parseContent,
  serializeContent,
  extractPlainText,
  rebuildContent,
} from "../contentBlockUtils";
import type { Attachment } from "../../attachment/types";

describe("parseContent", () => {
  it("returns null for plain text", () => {
    expect(parseContent("Hello world")).toBeNull();
  });

  it("returns null for empty string", () => {
    expect(parseContent("")).toBeNull();
  });

  it("returns null for non-ContentBlock JSON", () => {
    expect(parseContent('{"foo":"bar"}')).toBeNull();
    expect(parseContent("42")).toBeNull();
    expect(parseContent('"hello"')).toBeNull();
  });

  it("returns null for empty array", () => {
    expect(parseContent("[]")).toBeNull();
  });

  it("returns array for valid ContentBlock JSON", () => {
    const json = JSON.stringify([{ type: "text", text: "hello" }]);
    const result = parseContent(json);
    expect(result).not.toBeNull();
    expect(result!.length).toBe(1);
    expect(result![0].type).toBe("text");
  });

  it("returns array for mixed ContentBlock JSON", () => {
    const json = JSON.stringify([
      { type: "text", text: "look at this" },
      {
        type: "image",
        attachment: {
          attachmentId: "abc.png",
          mediaType: "image/png",
          bytes: 1234,
        },
      },
    ]);
    const result = parseContent(json);
    expect(result).not.toBeNull();
    expect(result!.length).toBe(2);
    expect(result![0].type).toBe("text");
    expect(result![1].type).toBe("image");
  });

  it("returns array even with unknown type (forward compat)", () => {
    const json = JSON.stringify([{ type: "audio", data: "..." }]);
    const result = parseContent(json);
    expect(result).not.toBeNull();
    expect(result!.length).toBe(1);
  });

  it("returns null for array without type field", () => {
    const json = JSON.stringify([{ text: "hello" }]);
    expect(parseContent(json)).toBeNull();
  });
});

describe("serializeContent", () => {
  it("round-trips with parseContent", () => {
    const blocks = [{ type: "text" as const, text: "hello" }];
    const serialized = serializeContent(blocks);
    const parsed = parseContent(serialized);
    expect(parsed).not.toBeNull();
    expect(parsed![0].type).toBe("text");
  });
});

describe("buildContentBlocks", () => {
  it("builds TextBlock from text only", () => {
    const blocks = buildContentBlocks("hello", []);
    expect(blocks.length).toBe(1);
    expect(blocks[0].type).toBe("text");
  });

  it("skips empty text", () => {
    const blocks = buildContentBlocks("  ", []);
    expect(blocks.length).toBe(0);
  });

  it("builds ImageBlock from image attachment", () => {
    const att: Attachment = {
      id: "att-1",
      kind: "image",
      file: new File([], "test.png", { type: "image/png" }),
      previewUrl: "blob://test",
      mimeType: "image/png",
      originalSizeBytes: 100,
    };
    const blocks = buildContentBlocks("hello", [att]);
    expect(blocks.length).toBe(2);
    expect(blocks[0].type).toBe("text");
    expect(blocks[1].type).toBe("image");
  });
});

describe("extractPlainText", () => {
  it("returns plain text as-is", () => {
    expect(extractPlainText("hello world")).toBe("hello world");
  });

  it("extracts text from ContentBlock JSON", () => {
    const json = JSON.stringify([
      { type: "text", text: "hello" },
      {
        type: "image",
        attachment: {
          attachmentId: "abc.png",
          mediaType: "image/png",
          bytes: 0,
        },
      },
    ]);
    expect(extractPlainText(json)).toBe("hello");
  });
});

describe("rebuildContent", () => {
  it("rebuilds with new text preserving images", () => {
    const original = JSON.stringify([
      { type: "text", text: "old text" },
      {
        type: "image",
        attachment: {
          attachmentId: "abc.png",
          mediaType: "image/png",
          bytes: 0,
        },
      },
    ]);
    const rebuilt = rebuildContent(original, "new text");
    const parsed = parseContent(rebuilt);
    expect(parsed).not.toBeNull();
    expect(parsed!.length).toBe(2);
    expect(parsed![0].type).toBe("text");
    expect(parsed![1].type).toBe("image");
  });

  it("falls back to plain text for old data", () => {
    const result = rebuildContent("old plain text", "new text");
    expect(result).toBe("new text");
  });
});
