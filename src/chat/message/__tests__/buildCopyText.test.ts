import { describe, expect, it } from "vitest";
import type { Message } from "../../core/chatTypes";
import { ChatErrorCode } from "../../core/chatErrors";
import { serializeContent } from "../../core/contentBlockUtils";
import { buildCopyText } from "../TurnTail";

function makeMessage(overrides: Partial<Message>): Message {
  return {
    id: "m1",
    conversationId: "c1",
    role: "user",
    content: "",
    tokens: null,
    timestamp: 1_700_000_000_000,
    parentId: null,
    isPinned: false,
    reasoning: null,
    siblingIndex: 0,
    siblingCount: null,
    providerId: null,
    modelId: null,
    modeId: null,
    turnIndex: null,
    iteration: 0,
    ...overrides,
  };
}

describe("buildCopyText：纯文本", () => {
  it("原样返回，不额外拼接", () => {
    expect(buildCopyText(makeMessage({ content: "hello world" }))).toBe(
      "hello world",
    );
  });

  it("空内容返回空串", () => {
    expect(buildCopyText(makeMessage({ content: "" }))).toBe("");
  });
});

describe("buildCopyText：多模态", () => {
  const multimodal = serializeContent([
    { type: "text", text: "看这两个附件" },
    {
      type: "image",
      attachment: {
        attachmentId: "a1",
        mediaType: "image/png",
        bytes: 1024,
        width: 10,
        height: 10,
        name: "shot.png",
      },
    },
    {
      type: "file",
      attachment: { attachmentId: "f1", name: "report.pdf", bytes: 2048 },
    },
  ]);

  it("按块派生可读文本，而不是复制原始 JSON", () => {
    const text = buildCopyText(makeMessage({ content: multimodal }));
    expect(text).toContain("看这两个附件");
    expect(text).toContain("[图片]");
    expect(text).toContain("[文件 report.pdf]");
  });

  it("不泄露 attachmentId 等内部字段", () => {
    const text = buildCopyText(makeMessage({ content: multimodal }));
    expect(text).not.toContain("attachmentId");
    expect(text).not.toContain('"type"');
  });
});

describe("buildCopyText：失败消息", () => {
  it("带上错误标题与详情，正文在前", () => {
    const text = buildCopyText(
      makeMessage({
        content: "hello",
        error: {
          code: ChatErrorCode.NetworkError,
          message: "socket hang up",
          retryAfterMs: null,
          retryable: true,
        },
      }),
    );
    const lines = text.split("\n");
    expect(lines[0]).toBe("hello");
    expect(lines[1].startsWith("[") && lines[1].endsWith("]")).toBe(true);
    expect(lines[2]).toBe("socket hang up");
  });

  it("正文为空时不留下前导空行", () => {
    const text = buildCopyText(
      makeMessage({
        content: "",
        error: {
          code: ChatErrorCode.Timeout,
          message: "timed out",
          retryAfterMs: null,
          retryable: true,
        },
      }),
    );
    expect(text.startsWith("[")).toBe(true);
  });

  it("多模态失败消息同样派生正文", () => {
    const text = buildCopyText(
      makeMessage({
        content: serializeContent([
          { type: "text", text: "hi" },
          {
            type: "file",
            attachment: { attachmentId: "f2", name: "a.txt", bytes: 1 },
          },
        ]),
        error: {
          code: ChatErrorCode.ServerError,
          message: "500",
          retryAfterMs: null,
          retryable: true,
        },
      }),
    );
    expect(text).toContain("hi");
    expect(text).toContain("[文件 a.txt]");
    expect(text).toContain("500");
  });
});
