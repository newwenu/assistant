import { describe, expect, it } from "vitest";
import { orbBusyFrom } from "../orbBusy";
import type { Message, ToolCallView } from "../../chat/core/chatTypes";

function call(callId: string, status: ToolCallView["status"]): ToolCallView {
  return { callId, name: "shell", status };
}

function message(toolCalls?: ToolCallView[]): Message {
  return {
    id: "m1",
    conversationId: "c1",
    role: "assistant",
    content: "",
    tokens: null,
    timestamp: 0,
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
    toolCalls,
  };
}

describe("orbBusyFrom", () => {
  it("流式进行中即为 busy", () => {
    expect(orbBusyFrom(true, [])).toBe(true);
  });

  it("存在执行中的工具调用即为 busy", () => {
    expect(orbBusyFrom(false, [message([call("a", "running")])])).toBe(true);
  });

  it("存在等待用户的工具调用即为 busy", () => {
    expect(orbBusyFrom(false, [message([call("a", "awaiting_user")])])).toBe(
      true,
    );
  });

  it("存在参数生成中的工具调用即为 busy", () => {
    expect(orbBusyFrom(false, [message([call("a", "preparing")])])).toBe(true);
  });

  it("无流式且工具调用全部收敛时不为 busy", () => {
    expect(
      orbBusyFrom(false, [
        message([call("a", "success"), call("b", "cancelled")]),
        message(),
      ]),
    ).toBe(false);
    expect(orbBusyFrom(false, [])).toBe(false);
  });
});
