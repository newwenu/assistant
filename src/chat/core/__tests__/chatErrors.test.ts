import { describe, expect, it } from "vitest";
import {
  ChatErrorCategory,
  ChatErrorCode,
  isChatError,
  toChatError,
} from "../chatErrors";
import { translateKey } from "../../../i18n";
import type { ProviderErrorPayloadDto } from "../../../types/protocol.contract";

const timeoutPayload: ProviderErrorPayloadDto = {
  code: "Timeout",
  message: "Request timeout: No chunk received for 30s (stream idle)",
  retryAfterMs: null,
  retryable: true,
  httpStatus: null,
  provider: null,
  providerCode: null,
};

const unknownErrorMessage = translateKey("chat.error.unknown_error");

describe("isChatError — 与生成契约的可空字段对齐", () => {
  it("接受三个可选字段全为 null 的 wire 载荷", () => {
    expect(isChatError(timeoutPayload)).toBe(true);
  });

  it("仍接受键缺失（undefined）的历史载荷", () => {
    expect(
      isChatError({ code: "Timeout", message: "boom", retryable: true }),
    ).toBe(true);
  });

  it("接受 details 为 null", () => {
    expect(
      isChatError({ code: "StreamError", retryable: false, details: null }),
    ).toBe(true);
  });

  it("拒绝类型错误的可选字段", () => {
    expect(
      isChatError({ code: "Timeout", retryable: true, httpStatus: "429" }),
    ).toBe(false);
    expect(
      isChatError({ code: "Timeout", retryable: true, provider: 42 }),
    ).toBe(false);
    expect(
      isChatError({ code: "Timeout", retryable: true, providerCode: {} }),
    ).toBe(false);
    expect(
      isChatError({ code: "Timeout", retryable: true, details: "x" }),
    ).toBe(false);
  });

  it("仍拒绝缺 retryable 与非对象输入", () => {
    expect(isChatError({ code: "Timeout" })).toBe(false);
    expect(isChatError(null)).toBe(false);
    expect(isChatError("boom")).toBe(false);
    expect(isChatError(new Error("boom"))).toBe(false);
  });
});

describe("toChatError — 归一化", () => {
  it("wire 载荷保留后端码、文案与可重试语义", () => {
    const err = toChatError(timeoutPayload);
    expect(err.code).toBe(ChatErrorCode.Timeout);
    expect(err.message).toBe(timeoutPayload.message);
    expect(err.retryable).toBe(true);
    expect(err.category).toBe(ChatErrorCategory.Timeout);
    expect(err.retryAfterMs).toBeNull();
    expect(err.httpStatus).toBeNull();
    expect(err.message).not.toBe(unknownErrorMessage);
  });

  it("带 HTTP 元信息的载荷逐字段保留", () => {
    const err = toChatError({
      code: "RateLimited",
      message: "rate limited",
      retryAfterMs: 30000,
      retryable: true,
      httpStatus: 429,
      provider: "openai",
      providerCode: "rate_limit_exceeded",
    });
    expect(err.code).toBe(ChatErrorCode.RateLimited);
    expect(err.message).toBe("rate limited");
    expect(err.retryAfterMs).toBe(30000);
    expect(err.retryable).toBe(true);
    expect(err.httpStatus).toBe(429);
    expect(err.provider).toBe("openai");
    expect(err.providerCode).toBe("rate_limit_exceeded");
    expect(err.category).toBe(ChatErrorCategory.RateLimit);
  });

  it("details 为 null 时不产生空 details", () => {
    const err = toChatError({
      code: "Timeout",
      message: "boom",
      retryable: true,
      details: null,
    });
    expect(err.details).toBeUndefined();
  });

  it("字符串输入保留文案并归一为 InternalError", () => {
    const err = toChatError("boom");
    expect(err.code).toBe(ChatErrorCode.InternalError);
    expect(err.message).toBe("boom");
    expect(err.retryable).toBe(false);
    expect(err.category).toBe(ChatErrorCategory.Unknown);
  });

  it("无法判别的输入仍走兜底文案", () => {
    for (const input of [null, {}, 123]) {
      const err = toChatError(input);
      expect(err.code).toBe(ChatErrorCode.InternalError);
      expect(err.message).toBe(unknownErrorMessage);
      expect(err.retryable).toBe(false);
      expect(err.category).toBe(ChatErrorCategory.Unknown);
    }
  });
});
