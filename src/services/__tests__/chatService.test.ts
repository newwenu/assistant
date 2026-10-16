import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
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

  describe("getConversationTokenUsage", () => {
    it("returns business type with totalTokens/contextWindowTokens", async () => {
      const row = {
        totalTokens: 100,
        contextWindowTokens: 128000,
        percentage: 0.00078,
      };
      mockInvoke.mockResolvedValue(row);
      const result = await chatService.getConversationTokenUsage("c1", 128000);
      expect(result).toEqual({
        totalTokens: 100,
        contextWindowTokens: 128000,
        percentage: 0.00078,
      });
    });
  });
});
