import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  contextUsageFromSummary,
  contextUsageFromTokenUsage,
  fetchTokenUsage,
} from "../chatUtils";
import type { ContextUsage, TokenUsageSummary } from "../chatTypes";
import { chatService } from "@/services/chatService";

vi.mock("@/services/chatService", () => ({
  chatService: {
    getConversationTokenUsage: vi.fn(),
  },
}));

function summary(
  partial: Partial<TokenUsageSummary["occupancy"]>,
): TokenUsageSummary {
  return { occupancy: partial };
}

describe("contextUsageFromSummary", () => {
  it("marks hasUsage=false when the branch has no record", () => {
    const usage = contextUsageFromSummary(summary({}));
    expect(usage.hasUsage).toBe(false);
    expect(usage.usedTokens).toBe(0);
    expect(usage.maxTokens).toBeUndefined();
    expect(usage.percentage).toBeUndefined();
  });

  it("keeps the window for an empty branch so 0 can be rendered against it", () => {
    const usage = contextUsageFromSummary(
      summary({ contextWindowTokens: 128_000, percentage: 0 }),
    );
    expect(usage.hasUsage).toBe(false);
    expect(usage.usedTokens).toBe(0);
    expect(usage.maxTokens).toBe(128_000);
    expect(usage.percentage).toBe(0);
    expect(usage.estimated).toBeUndefined();
  });

  it("shows absolute number only when the window is unknown", () => {
    const usage = contextUsageFromSummary(summary({ usedTokens: 12_000 }));
    expect(usage.hasUsage).toBe(true);
    expect(usage.usedTokens).toBe(12_000);
    expect(usage.maxTokens).toBeUndefined();
    expect(usage.percentage).toBeUndefined();
  });

  it("computes percentage when the window is known", () => {
    const usage = contextUsageFromSummary(
      summary({
        usedTokens: 64_000,
        contextWindowTokens: 128_000,
        percentage: 0.5,
      }),
    );
    expect(usage.maxTokens).toBe(128_000);
    expect(usage.percentage).toBe(0.5);
  });

  it("keeps cumulative separate from occupancy", () => {
    const usage = contextUsageFromSummary({
      occupancy: { usedTokens: 1_000 },
      cumulative: {
        totalTokens: 9_000,
        promptTokens: 5_000,
        completionTokens: 4_000,
      },
    });
    expect(usage.usedTokens).toBe(1_000);
    expect(usage.cumulativeTokens).toBe(9_000);
  });

  it("passes the estimated flag through", () => {
    const usage = contextUsageFromSummary(
      summary({ usedTokens: 1_000, estimated: true }),
    );
    expect(usage.estimated).toBe(true);
  });

  it("keeps cumulative undefined when the conversation has no records", () => {
    const usage = contextUsageFromSummary(summary({}));
    expect(usage.hasUsage).toBe(false);
    expect(usage.cumulativeTokens).toBeUndefined();
  });

  it("marks a projection estimate with its source", () => {
    const usage = contextUsageFromSummary(
      summary({
        usedTokens: 4_000,
        contextWindowTokens: 128_000,
        percentage: 4_000 / 128_000,
        estimated: true,
        estimatedSource: "projection",
      }),
    );
    expect(usage.hasUsage).toBe(true);
    expect(usage.usedTokens).toBe(4_000);
    expect(usage.estimated).toBe(true);
    expect(usage.estimatedSource).toBe("projection");
    expect(usage.cumulativeTokens).toBeUndefined();
  });

  it("marks a stored estimate with its source", () => {
    const usage = contextUsageFromSummary(
      summary({
        usedTokens: 1_000,
        estimated: true,
        estimatedSource: "record",
      }),
    );
    expect(usage.estimatedSource).toBe("record");
  });
});

describe("contextUsageFromTokenUsage", () => {
  it("ignores a zero window instead of using it as denominator", () => {
    const usage = contextUsageFromTokenUsage({
      promptTokens: 500,
      completionTokens: 20,
      totalTokens: 520,
      contextWindowTokens: 0,
    });
    expect(usage.maxTokens).toBeUndefined();
    expect(usage.percentage).toBeUndefined();
    expect(usage.usedTokens).toBe(500);
  });
});

describe("fetchTokenUsage", () => {
  beforeEach(() => {
    vi.mocked(chatService.getConversationTokenUsage).mockReset();
  });

  it("applies the summary when the conversation is still active", async () => {
    vi.mocked(chatService.getConversationTokenUsage).mockResolvedValue(
      summary({
        usedTokens: 2_000,
        contextWindowTokens: 128_000,
        percentage: 1 / 64,
      }),
    );
    const setContextUsage = vi.fn<(usage: ContextUsage) => void>();

    fetchTokenUsage("c1", { isActiveConv: () => true, setContextUsage });
    await vi.waitFor(() => expect(setContextUsage).toHaveBeenCalledTimes(1));
    expect(setContextUsage.mock.calls[0][0].usedTokens).toBe(2_000);
  });

  it("ignores a stale response after switching to another conversation", async () => {
    vi.mocked(chatService.getConversationTokenUsage).mockResolvedValue(
      summary({ usedTokens: 2_000 }),
    );
    const setContextUsage = vi.fn();

    fetchTokenUsage("c1", { isActiveConv: () => false, setContextUsage });
    await vi.waitFor(() => expect(setContextUsage).not.toHaveBeenCalled());
  });

  it("ignores an earlier request that resolves after a newer one", async () => {
    let resolveFirst: (value: TokenUsageSummary | null) => void = () => {};
    let resolveSecond: (value: TokenUsageSummary | null) => void = () => {};
    vi.mocked(chatService.getConversationTokenUsage)
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveFirst = resolve;
          }),
      )
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveSecond = resolve;
          }),
      );
    const setContextUsage = vi.fn<(usage: ContextUsage) => void>();
    const deps = { isActiveConv: () => true, setContextUsage };

    fetchTokenUsage("c1", deps);
    fetchTokenUsage("c1", deps);

    resolveSecond(summary({ usedTokens: 3_000 }));
    await vi.waitFor(() => expect(setContextUsage).toHaveBeenCalledTimes(1));
    expect(setContextUsage.mock.calls[0][0].usedTokens).toBe(3_000);

    resolveFirst(summary({ usedTokens: 1_000 }));
    await Promise.resolve();
    await Promise.resolve();
    expect(setContextUsage).toHaveBeenCalledTimes(1);
  });

  it("ignores a null summary", async () => {
    vi.mocked(chatService.getConversationTokenUsage).mockResolvedValue(null);
    const setContextUsage = vi.fn();

    fetchTokenUsage("c1", { isActiveConv: () => true, setContextUsage });
    await Promise.resolve();
    await Promise.resolve();
    expect(setContextUsage).not.toHaveBeenCalled();
  });
});
