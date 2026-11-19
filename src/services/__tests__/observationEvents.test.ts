import { describe, expect, it, vi } from "vitest";
import type { AgentNodeEventDto } from "../../types/protocol.contract";
import { ChatEvent } from "../../chat/core/chatTypes";
import { useObservationStore } from "../../stores/agent/observation/store";
import { observationKey } from "../../stores/agent/observation/types";
import { ensureObservationEvents } from "../observationEvents";

interface RawEvent {
  payload: unknown;
}

const listenMock = vi.hoisted(() => {
  const handlers = new Map<string, (event: RawEvent) => void>();
  const calls: string[] = [];
  return {
    handlers,
    calls,
    listen: (name: string, handler: (event: RawEvent) => void) => {
      calls.push(name);
      handlers.set(name, handler);
      return Promise.resolve(() => {
        handlers.delete(name);
      });
    },
  };
});

vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock.listen }));

function nodeEvent(): AgentNodeEventDto {
  return {
    conversationId: "conv-1",
    turnId: "conv-1:1",
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

describe("观测订阅 — 主窗口侧接线（I55 §6.3 第 1 条）", () => {
  it("幂等：重复调用只建立一条监听，事件只折叠一次", () => {
    ensureObservationEvents();
    ensureObservationEvents();

    expect(listenMock.calls).toEqual([ChatEvent.AgentNodeEvent]);

    const handler = listenMock.handlers.get(ChatEvent.AgentNodeEvent);
    expect(handler).toBeDefined();

    const event = nodeEvent();
    handler!({ payload: event });

    const key = observationKey(event.conversationId, event.turnId);
    const instance = useObservationStore.getState().observations.get(key);

    expect(instance).toBeDefined();
    expect(instance!.timeline).toHaveLength(1);
  });
});
