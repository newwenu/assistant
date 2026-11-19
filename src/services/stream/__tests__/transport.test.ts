import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  AgentNodeEventDto,
  AgentToolCallDto,
} from "../../../types/protocol.contract";
import { ChatEvent } from "../../../chat/core/chatTypes";
import {
  subscribeStreamTransport,
  type StreamTransportHandlers,
} from "../transport";

interface RawEvent {
  payload: unknown;
}

const listenMock = vi.hoisted(() => {
  const handlers = new Map<string, (event: RawEvent) => void>();
  return {
    handlers,
    listen: (name: string, handler: (event: RawEvent) => void) => {
      handlers.set(name, handler);
      return Promise.resolve(() => {
        handlers.delete(name);
      });
    },
  };
});

vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock.listen }));

function emit(name: string, payload: unknown): void {
  const handler = listenMock.handlers.get(name);
  if (!handler) throw new Error(`未注册监听：${name}`);
  handler({ payload });
}

function toolCall(conversationId: string): AgentToolCallDto {
  return {
    conversationId,
    turnId: `${conversationId}:1`,
    iteration: 1,
    callId: "call-1",
    name: "read_file",
    payload: { phase: "started", argsPreview: null },
  };
}

function nodeEvent(conversationId: string): AgentNodeEventDto {
  return {
    conversationId,
    turnId: `${conversationId}:1`,
    seq: 1,
    iteration: 1,
    node: "model",
    state: "running",
    transition: true,
    atMs: 10,
    agentId: "root",
    parentTurnId: null,
    detail: null,
  };
}

function createHandlers(activeConversationId: string | null) {
  return {
    getActiveConversationId: vi.fn(() => activeConversationId),
    onStreamStart: vi.fn(),
    onUserMessageConfirmed: vi.fn(),
    onSessionState: vi.fn(),
    onToolCall: vi.fn(),
    onNodeEvent: vi.fn(),
    onTextDelta: vi.fn(),
    onDeltaEvent: vi.fn(),
  } satisfies StreamTransportHandlers;
}

beforeEach(() => {
  listenMock.handlers.clear();
});

describe("传输层 — 后台会话事件不丢（G6）", () => {
  it("非活跃会话的 agent-tool-call 仍透传给编排层", () => {
    const handlers = createHandlers("conv-active");
    const dispose = subscribeStreamTransport(handlers);

    emit(ChatEvent.AgentToolCall, toolCall("conv-background"));

    expect(handlers.onToolCall).toHaveBeenCalledTimes(1);
    expect(handlers.onToolCall).toHaveBeenCalledWith(
      expect.objectContaining({ conversationId: "conv-background" }),
    );

    dispose();
  });

  it("非活跃会话的 agent-node-event 仍透传给编排层", () => {
    const handlers = createHandlers("conv-active");
    const dispose = subscribeStreamTransport(handlers);

    emit(ChatEvent.AgentNodeEvent, nodeEvent("conv-background"));

    expect(handlers.onNodeEvent).toHaveBeenCalledTimes(1);
    expect(handlers.onNodeEvent).toHaveBeenCalledWith(
      expect.objectContaining({ conversationId: "conv-background", seq: 1 }),
    );

    dispose();
  });

  it("chat-user-message-confirmed 仍按活跃会话过滤", () => {
    const handlers = createHandlers("conv-active");
    const dispose = subscribeStreamTransport(handlers);

    emit(ChatEvent.UserMessageConfirmed, {
      conversationId: "conv-background",
      userMessageId: "m-1",
      userMessageContent: "hi",
      turnIndex: 1,
    });
    expect(handlers.onUserMessageConfirmed).not.toHaveBeenCalled();

    emit(ChatEvent.UserMessageConfirmed, {
      conversationId: "conv-active",
      userMessageId: "m-2",
      userMessageContent: "hello",
      turnIndex: 1,
    });
    expect(handlers.onUserMessageConfirmed).toHaveBeenCalledTimes(1);

    dispose();
  });

  it("节点事件监听不做任何会话过滤（无活跃会话时也透传）", () => {
    const handlers = createHandlers(null);
    const dispose = subscribeStreamTransport(handlers);

    emit(ChatEvent.AgentNodeEvent, nodeEvent("conv-any"));

    expect(handlers.onNodeEvent).toHaveBeenCalledTimes(1);

    dispose();
  });
});
