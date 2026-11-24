import { describe, expect, it } from "vitest";
import { fallbackTitleFromUserMessage } from "../title";

describe("fallbackTitleFromUserMessage", () => {
  it("derives a non-empty title from an attachment-only message", () => {
    const json = JSON.stringify([
      { type: "image", attachment: { attachmentId: "a" } },
    ]);
    expect(fallbackTitleFromUserMessage(json)).toBe("[图片]");
  });

  it("uses the first non-empty derived line for mixed content", () => {
    const json = JSON.stringify([
      { type: "text", text: "look at this" },
      { type: "image", attachment: { attachmentId: "a" } },
    ]);
    expect(fallbackTitleFromUserMessage(json)).toBe("look at this");
  });
});
