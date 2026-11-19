import { describe, it, expect } from "vitest";
import type { Message, ToolCallView } from "../../core/chatTypes";
import { makeMessage } from "../../../test-utils/messageFixtures";
import { pickActiveTool } from "../activeTool";

function view(
  callId: string,
  status: ToolCallView["status"],
  name = callId,
): ToolCallView {
  return { callId, name, status, startedAt: 0, finishedAt: null };
}

function assistant(id: string, toolCalls: ToolCallView[]): Message {
  return makeMessage(id, { role: "assistant", toolCalls });
}

describe("pickActiveTool", () => {
  it("优先最后一个 running（S6）", () => {
    const messages = [
      assistant("m1", [view("a", "success")]),
      assistant("m2", [view("b", "running", "grep")]),
      assistant("m3", [view("c", "running", "shell")]),
    ];
    expect(pickActiveTool(messages)?.name).toBe("shell");
  });

  it("无 running 时取最后一个 preparing", () => {
    const messages = [
      assistant("m1", [view("a", "preparing")]),
      assistant("m2", [view("b", "preparing")]),
    ];
    expect(pickActiveTool(messages)?.callId).toBe("b");
  });

  it("awaiting_user 不进入指示器（S7）", () => {
    expect(
      pickActiveTool([assistant("m1", [view("a", "awaiting_user")])]),
    ).toBeNull();
  });

  it("非 assistant 行与空列表返回 null", () => {
    expect(pickActiveTool([])).toBeNull();
    expect(
      pickActiveTool([
        makeMessage("m1", { role: "user", toolCalls: [view("a", "running")] }),
      ]),
    ).toBeNull();
  });
});
