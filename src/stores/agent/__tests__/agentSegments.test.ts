import { describe, it, expect, beforeEach } from "vitest";
import { createAgentSegments } from "../agentSegments";
import { useChatStreamStore } from "../../chatStreamStore";
import type { AgentToolCall, Message } from "../../../chat/core/chatTypes";

type ToolPayload = AgentToolCall["payload"];

function startedCall(iteration: number, callId = "call-1"): AgentToolCall {
  return {
    conversationId: "conv-1",
    turnId: "conv-1:1",
    iteration,
    callId,
    name: "read_file",
    payload: { phase: "started" },
  };
}

function finishedCall(iteration: number, callId = "call-1"): AgentToolCall {
  return {
    conversationId: "conv-1",
    turnId: "conv-1:1",
    iteration,
    callId,
    name: "read_file",
    payload: { phase: "finished", result: "ok" } as ToolPayload,
  };
}

describe("agentSegments — 一次迭代 = 一条消息 = 一个气泡", () => {
  let messages: Message[] = [];
  let segments: ReturnType<typeof createAgentSegments>;

  const set = (partial: Partial<{ messages: Message[] }>) => {
    if (partial.messages) messages = partial.messages;
  };
  const get = () => ({
    messages,
    currentConversationId: "conv-1",
  });

  beforeEach(() => {
    messages = [];
    useChatStreamStore.getState().cancelStreaming();
    segments = createAgentSegments(set, get);
  });

  function beginTurn(assistantMsgId: string) {
    messages = [
      {
        id: assistantMsgId,
        conversationId: "conv-1",
        role: "assistant",
        content: "",
        tokens: null,
        timestamp: Date.now(),
        parentId: "msg-user",
        isPinned: false,
        reasoning: null,
        siblingIndex: 0,
        siblingCount: null,
        providerId: null,
        modelId: null,
      },
    ];
    useChatStreamStore.getState().beginStream(assistantMsgId);
    segments.reset(assistantMsgId);
  }

  it("每个迭代各自成段，内容不会串到同一条消息里", () => {
    beginTurn("temp-a1");

    segments.appendDelta(1, "first");
    segments.commitSegment();

    segments.appendDelta(2, "second");
    segments.commitSegment();

    expect(messages).toHaveLength(2);
    expect(messages[0].content).toBe("first");
    expect(messages[1].content).toBe("second");
    expect(messages[0].id).not.toBe(messages[1].id);
  });

  it("迭代边界切段时清空缓冲，第二段不继承第一段正文", () => {
    beginTurn("temp-a1");

    segments.appendDelta(1, "first");
    segments.appendDelta(2, "second");

    expect(messages[0].content).toBe("first");
    expect(useChatStreamStore.getState().streamingContent).toBe("second");
    expect(useChatStreamStore.getState().streamingIteration).toBe(2);
  });

  it("同一迭代内的多次增量仍落在同一条消息", () => {
    beginTurn("temp-a1");

    segments.appendDelta(1, "a");
    segments.appendDelta(1, "b");
    segments.commitSegment();

    expect(messages).toHaveLength(1);
    expect(messages[0].content).toBe("ab");
  });

  it("落后于当前段的旧迭代事件被丢弃", () => {
    beginTurn("temp-a1");

    segments.appendDelta(1, "first");
    segments.appendDelta(2, "second");
    segments.appendDelta(1, "late");

    expect(messages[0].content).toBe("first");
    expect(useChatStreamStore.getState().streamingContent).toBe("second");
  });

  it("工具卡片按 iteration 归到发起它的那条消息", () => {
    beginTurn("temp-a1");

    segments.appendDelta(1, "first");
    segments.applyToolCall(1, startedCall(1, "call-1"));
    segments.appendDelta(2, "second");
    segments.applyToolCall(2, finishedCall(2, "call-2"));

    expect(messages[0].toolCalls?.[0]?.callId).toBe("call-1");
    expect(messages[1].toolCalls?.[0]?.callId).toBe("call-2");
  });

  it("settleToolCalls 把挂起的调用收敛为 cancelled", () => {
    beginTurn("temp-a1");

    segments.appendDelta(1, "first");
    segments.applyToolCall(1, startedCall(1));
    segments.settleToolCalls();

    expect(messages[0].toolCalls?.[0]?.status).toBe("cancelled");
  });

  it("markUnknownSegment 后首个增量开新段，不覆盖已落库正文", () => {
    beginTurn("temp-a1");
    segments.appendDelta(1, "first");
    segments.commitSegment();

    useChatStreamStore.getState().cancelStreaming();
    segments.markUnknownSegment();

    segments.appendDelta(1, "resumed");

    expect(messages).toHaveLength(2);
    expect(messages[0].content).toBe("first");
    expect(useChatStreamStore.getState().streamingContent).toBe("resumed");
  });
});
