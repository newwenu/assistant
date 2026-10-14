import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { chatService } from "../chatService";

const mockInvoke = invoke as unknown as ReturnType<typeof vi.fn>;

describe("chatService — 契约测试", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  describe("listConversations", () => {
    it("calls invoke with command 'list_conversations' and { providerId: null }", async () => {
      mockInvoke.mockResolvedValue([]);
      await chatService.listConversations();
      expect(mockInvoke).toHaveBeenCalledWith("list_conversations", {
        providerId: null,
      });
    });

    it("casts result to Conversation[] and returns as-is when non-null", async () => {
      const convs = [{ id: "c1", title: "hello" }];
      mockInvoke.mockResolvedValue(convs);
      const result = await chatService.listConversations();
      expect(result).toBe(convs);
    });

    it("returns [] when invoke returns null/undefined (|| [] fallback)", async () => {
      mockInvoke.mockResolvedValue(null);
      const result = await chatService.listConversations();
      expect(result).toEqual([]);
    });
  });

  describe("listProviders", () => {
    it("calls invoke with command 'list_providers' and no args", async () => {
      mockInvoke.mockResolvedValue([]);
      await chatService.listProviders();
      expect(mockInvoke).toHaveBeenCalledWith("list_providers");
    });

    it("returns casted result with || [] fallback", async () => {
      mockInvoke.mockResolvedValue([
        { id: "openai", default_model: "gpt-4" },
      ]);
      const result = await chatService.listProviders();
      expect(result).toEqual([{ id: "openai", default_model: "gpt-4" }]);
    });

    it("returns [] on null", async () => {
      mockInvoke.mockResolvedValue(null);
      const result = await chatService.listProviders();
      expect(result).toEqual([]);
    });
  });

  describe("createConversation", () => {
    it("calls invoke with command 'create_conversation' and providerId, modelId", async () => {
      mockInvoke.mockResolvedValue("conv-1");
      const result = await chatService.createConversation("openai", "gpt-4");
      expect(mockInvoke).toHaveBeenCalledWith("create_conversation", {
        providerId: "openai",
        modelId: "gpt-4",
        modeId: undefined,
      });
      expect(result).toBe("conv-1");
    });

    it("passes modeId when provided", async () => {
      mockInvoke.mockResolvedValue("conv-2");
      await chatService.createConversation("openai", "gpt-4", "agent");
      expect(mockInvoke).toHaveBeenCalledWith("create_conversation", {
        providerId: "openai",
        modelId: "gpt-4",
        modeId: "agent",
      });
    });
  });

  describe("getConversationPaginated", () => {
    it("calls invoke with command 'get_conversation_paginated' and id, limit, beforeTimestamp", async () => {
      const detail = {
        conversation: { id: "c1" },
        messages: [],
        total_count: 0,
      };
      mockInvoke.mockResolvedValue(detail);
      const result = await chatService.getConversationPaginated("c1", 50, null);
      expect(mockInvoke).toHaveBeenCalledWith("get_conversation_paginated", {
        id: "c1",
        limit: 50,
        beforeTimestamp: null,
      });
      expect(result).toBe(detail);
    });

    it("returns null when invoke returns null", async () => {
      mockInvoke.mockResolvedValue(null);
      const result = await chatService.getConversationPaginated("c1", 50, null);
      expect(result).toBeNull();
    });

    it("passes beforeTimestamp when provided", async () => {
      mockInvoke.mockResolvedValue(null);
      await chatService.getConversationPaginated("c1", 50, 12345);
      expect(mockInvoke).toHaveBeenCalledWith("get_conversation_paginated", {
        id: "c1",
        limit: 50,
        beforeTimestamp: 12345,
      });
    });

    it("accepts null id (matches loadMoreMessages passing state.currentConversationId)", async () => {
      mockInvoke.mockResolvedValue(null);
      await chatService.getConversationPaginated(null, 50, 12345);
      expect(mockInvoke).toHaveBeenCalledWith("get_conversation_paginated", {
        id: null,
        limit: 50,
        beforeTimestamp: 12345,
      });
    });
  });

  describe("getConversationTokenUsage", () => {
    it("calls invoke with command 'get_conversation_token_usage' and conversationId, contextWindowTokens", async () => {
      const summary = {
        total_tokens: 100,
        context_window_tokens: 128000,
        percentage: 0.00078,
      };
      mockInvoke.mockResolvedValue(summary);
      const result = await chatService.getConversationTokenUsage(
        "c1",
        128000,
      );
      expect(mockInvoke).toHaveBeenCalledWith("get_conversation_token_usage", {
        conversationId: "c1",
        contextWindowTokens: 128000,
      });
      expect(result).toEqual(summary);
    });

    it("passes undefined contextWindowTokens", async () => {
      mockInvoke.mockResolvedValue({});
      await chatService.getConversationTokenUsage("c1", undefined);
      expect(mockInvoke).toHaveBeenCalledWith("get_conversation_token_usage", {
        conversationId: "c1",
        contextWindowTokens: undefined,
      });
    });
  });

  describe("deleteConversation", () => {
    it("calls invoke with command 'delete_conversation' and { id }", async () => {
      mockInvoke.mockResolvedValue(undefined);
      await chatService.deleteConversation("c1");
      expect(mockInvoke).toHaveBeenCalledWith("delete_conversation", {
        id: "c1",
      });
    });
  });

  describe("renameConversation", () => {
    it("calls invoke with command 'rename_conversation' and id, title, titleSource", async () => {
      mockInvoke.mockResolvedValue(undefined);
      await chatService.renameConversation("c1", "new title", "user");
      expect(mockInvoke).toHaveBeenCalledWith("rename_conversation", {
        id: "c1",
        title: "new title",
        titleSource: "user",
      });
    });
  });

  describe("updateConversationModel", () => {
    it("calls invoke with command 'update_conversation_model' and id, modelId", async () => {
      mockInvoke.mockResolvedValue(undefined);
      await chatService.updateConversationModel("c1", "gpt-4o");
      expect(mockInvoke).toHaveBeenCalledWith("update_conversation_model", {
        id: "c1",
        modelId: "gpt-4o",
      });
    });
  });

  describe("sendChatMessage", () => {
    it("calls invoke with command 'send_chat_message' and conversationId, content, modeId", async () => {
      mockInvoke.mockResolvedValue(undefined);
      await chatService.sendChatMessage("c1", "hello", "agent");
      expect(mockInvoke).toHaveBeenCalledWith("send_chat_message", {
        conversationId: "c1",
        content: "hello",
        modeId: "agent",
      });
    });

    it("passes undefined modeId when omitted", async () => {
      mockInvoke.mockResolvedValue(undefined);
      await chatService.sendChatMessage("c1", "hello");
      expect(mockInvoke).toHaveBeenCalledWith("send_chat_message", {
        conversationId: "c1",
        content: "hello",
        modeId: undefined,
      });
    });
  });

  describe("cancelChat", () => {
    it("calls invoke with command 'cancel_chat' and conversationId", async () => {
      mockInvoke.mockResolvedValue(undefined);
      await chatService.cancelChat("c1");
      expect(mockInvoke).toHaveBeenCalledWith("cancel_chat", {
        conversationId: "c1",
      });
    });
  });

  describe("incrementGeneration", () => {
    it("calls invoke with command 'increment_generation' and { id }", async () => {
      mockInvoke.mockResolvedValue(undefined);
      await chatService.incrementGeneration("c1");
      expect(mockInvoke).toHaveBeenCalledWith("increment_generation", {
        id: "c1",
      });
    });
  });

  describe("deleteMessagePersist", () => {
    it("calls invoke with command 'delete_message_persist' and messageId", async () => {
      mockInvoke.mockResolvedValue(undefined);
      await chatService.deleteMessagePersist("m1");
      expect(mockInvoke).toHaveBeenCalledWith("delete_message_persist", {
        messageId: "m1",
      });
    });
  });

  describe("editMessage", () => {
    it("calls invoke with command 'edit_message' and messageId, newContent", async () => {
      mockInvoke.mockResolvedValue(undefined);
      await chatService.editMessage("m1", "edited content");
      expect(mockInvoke).toHaveBeenCalledWith("edit_message", {
        messageId: "m1",
        newContent: "edited content",
      });
    });
  });

  describe("regenerateMessage", () => {
    it("calls invoke with command 'regenerate_message' and conversationId, messageId", async () => {
      mockInvoke.mockResolvedValue(undefined);
      await chatService.regenerateMessage("c1", "m1");
      expect(mockInvoke).toHaveBeenCalledWith("regenerate_message", {
        conversationId: "c1",
        messageId: "m1",
      });
    });
  });

  describe("toggleConversationPin", () => {
    it("calls invoke with command 'toggle_conversation_pin' and { id }", async () => {
      mockInvoke.mockResolvedValue(undefined);
      await chatService.toggleConversationPin("c1");
      expect(mockInvoke).toHaveBeenCalledWith("toggle_conversation_pin", {
        id: "c1",
      });
    });
  });

  describe("duplicateConversation", () => {
    it("calls invoke with command 'duplicate_conversation' and { id }", async () => {
      mockInvoke.mockResolvedValue(undefined);
      await chatService.duplicateConversation("c1");
      expect(mockInvoke).toHaveBeenCalledWith("duplicate_conversation", {
        id: "c1",
      });
    });
  });

  describe("batchDeleteConversations", () => {
    it("calls invoke with command 'batch_delete_conversations' and { ids }", async () => {
      mockInvoke.mockResolvedValue(undefined);
      await chatService.batchDeleteConversations(["c1", "c2"]);
      expect(mockInvoke).toHaveBeenCalledWith("batch_delete_conversations", {
        ids: ["c1", "c2"],
      });
    });
  });

  describe("purgeConversations", () => {
    it("calls invoke with command 'purge_conversations' and no args", async () => {
      mockInvoke.mockResolvedValue(undefined);
      await chatService.purgeConversations();
      expect(mockInvoke).toHaveBeenCalledWith("purge_conversations");
    });
  });
});