import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("../tauriInvoke", () => ({
  tauriInvoke: vi.fn(),
}));

import { tauriInvoke as invoke } from "../tauriInvoke";
import { chatService } from "../chatService";

const mockInvoke = vi.mocked(invoke);

describe("chatService", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  describe("listConversations", () => {
    it("returns [] when invoke returns null (fallback)", async () => {
      mockInvoke.mockResolvedValue(null);
      const result = await chatService.listConversations();
      expect(result).toEqual([]);
    });
  });

  describe("getConversationPaginated", () => {
    it("converts DTO to business type (totalCount from totalCount)", async () => {
      mockInvoke.mockResolvedValue({
        conversation: { id: "c1" },
        messages: [],
        totalCount: 42,
      });
      const result = await chatService.getConversationPaginated("c1", 50, null);
      expect(result).not.toBeNull();
      expect(result!.totalCount).toBe(42);
    });

    it("returns null when invoke returns null", async () => {
      mockInvoke.mockResolvedValue(null);
      const result = await chatService.getConversationPaginated("c1", 50, null);
      expect(result).toBeNull();
    });
  });

  describe("getConversationAllMessages", () => {
    it("returns ConversationDetail when invoke returns data", async () => {
      mockInvoke.mockResolvedValue({
        conversation: { id: "c1" },
        messages: [{ id: "m1", toolCalls: [] }],
        totalCount: 1,
      });
      const result = await chatService.getConversationAllMessages("c1");
      expect(result).not.toBeNull();
      expect(result!.messages).toHaveLength(1);
    });

    it("maps wire toolCalls into ToolCallView", async () => {
      mockInvoke.mockResolvedValue({
        conversation: { id: "c1" },
        messages: [
          {
            id: "m1",
            toolCalls: [
              {
                callId: "call-1",
                name: "read_file",
                arguments: { path: "a.txt" },
                status: "success",
                result: "ok",
              },
            ],
          },
        ],
        totalCount: 1,
      });
      const result = await chatService.getConversationAllMessages("c1");
      expect(result!.messages[0].toolCalls).toEqual([
        {
          callId: "call-1",
          name: "read_file",
          status: "success",
          arguments: { path: "a.txt" },
          resultText: "ok",
        },
      ]);
    });

    it("returns null when invoke returns null (fallback)", async () => {
      mockInvoke.mockResolvedValue(null);
      const result = await chatService.getConversationAllMessages("c1");
      expect(result).toBeNull();
    });
  });

  describe("getConversationTokenUsage", () => {
    it("returns occupancy/cumulative/cache groups unchanged", async () => {
      const row = {
        occupancy: {
          usedTokens: 100,
          completionTokens: 50,
          contextWindowTokens: 128000,
          percentage: 0.00078,
          estimated: false,
        },
        cumulative: {
          totalTokens: 150,
          promptTokens: 100,
          completionTokens: 50,
        },
        cache: { hitTokens: 20, denominatorTokens: 100, hitRate: 0.2 },
      };
      mockInvoke.mockResolvedValue(row);
      const result = await chatService.getConversationTokenUsage("c1");
      expect(result).toEqual(row);
    });

    it("keeps unknown window and empty groups absent", async () => {
      const row = { occupancy: { usedTokens: 100 } };
      mockInvoke.mockResolvedValue(row);
      const result = await chatService.getConversationTokenUsage("c1");
      expect(result).toEqual(row);
    });
  });
});
