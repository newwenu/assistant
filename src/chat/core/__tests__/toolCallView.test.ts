import { describe, it, expect } from "vitest";
import type {
  AgentToolCall,
  AgentToolCallPayload,
  Message,
} from "../chatTypes";
import type { MessageDto } from "../../../types/protocol.contract";
import {
  applyAgentToolCall,
  cancelPendingToolCalls,
  hasPendingToolCalls,
  toChatMessage,
  toolCallsFromHistory,
} from "../toolCallView";

const NOW = 1_700_000_000_000;

function call(
  payload: AgentToolCall["payload"],
  overrides: Partial<AgentToolCall> = {},
): AgentToolCall {
  return {
    conversationId: "conv-1",
    turnId: "turn-1",
    iteration: 1,
    callId: "call-1",
    name: "shell",
    payload,
    ...overrides,
  };
}

function approval(): Extract<
  AgentToolCallPayload,
  { phase: "awaitingApproval" }
>["approval"] {
  return {
    conversationId: "conv-1",
    turnId: "turn-1",
    callId: "call-1",
    toolName: "shell",
    arguments: { command: ["ls", "-la"] },
    intents: [],
    timeoutMs: 120_000,
    context: {
      tier: "os",
      behavior: { readOnly: false, destructive: false, openWorld: false },
      reasons: [],
      preview: null,
      extensions: [],
    },
  };
}

describe("applyAgentToolCall 五态映射", () => {
  it("started 建卡为 running 并记录起始时间", () => {
    const views = applyAgentToolCall([], call({ phase: "started" }), NOW);
    expect(views).toHaveLength(1);
    expect(views[0]).toMatchObject({
      callId: "call-1",
      name: "shell",
      status: "running",
      startedAt: NOW,
      finishedAt: null,
      resultText: null,
      argsText: "",
    });
  });

  it("argsDelta 累积原文，可解析时同步 arguments", () => {
    let views = applyAgentToolCall([], call({ phase: "started" }), NOW);
    views = applyAgentToolCall(
      views,
      call({ phase: "argsDelta", delta: '{"command":' }),
      NOW + 1,
    );
    expect(views[0].argsText).toBe('{"command":');
    expect(views[0].arguments).toBeUndefined();

    views = applyAgentToolCall(
      views,
      call({ phase: "argsDelta", delta: '["ls"]}' }),
      NOW + 2,
    );
    expect(views[0].argsText).toBe('{"command":["ls"]}');
    expect(views[0].arguments).toEqual({ command: ["ls"] });
    expect(views[0].status).toBe("running");
  });

  it("finished 收敛为 success 并带结果", () => {
    let views = applyAgentToolCall([], call({ phase: "started" }), NOW);
    views = applyAgentToolCall(
      views,
      call({
        phase: "finished",
        result: "ok",
        confirmationWaitMs: 1200,
      }),
      NOW + 10,
    );
    expect(views[0]).toMatchObject({
      status: "success",
      resultText: "ok",
      finishedAt: NOW + 10,
      confirmationWaitMs: 1200,
    });
  });

  it("failed 收敛为 error 并保留 errorKind", () => {
    let views = applyAgentToolCall([], call({ phase: "started" }), NOW);
    views = applyAgentToolCall(
      views,
      call({
        phase: "failed",
        error: "boom",
        errorKind: "business",
        confirmationWaitMs: null,
      }),
      NOW + 10,
    );
    expect(views[0]).toMatchObject({
      status: "error",
      resultText: "boom",
      errorKind: "business",
      finishedAt: NOW + 10,
    });
  });

  it("awaitingApproval 进入 awaiting_user，参数取自 approval", () => {
    let views = applyAgentToolCall([], call({ phase: "started" }), NOW);
    views = applyAgentToolCall(
      views,
      call({ phase: "awaitingApproval", approval: approval() }),
      NOW + 5,
    );
    expect(views[0].status).toBe("awaiting_user");
    expect(views[0].arguments).toEqual({ command: ["ls", "-la"] });
    expect(views[0].approval?.toolName).toBe("shell");
  });

  it("awaitingAnswer 进入 awaiting_user 并挂上提问载荷", () => {
    let views = applyAgentToolCall([], call({ phase: "started" }), NOW);
    views = applyAgentToolCall(
      views,
      call({
        phase: "awaitingAnswer",
        question: {
          conversationId: "conv-1",
          turnId: "turn-1",
          callId: "call-1",
          timeoutMs: 180_000,
          questions: [
            {
              question: "用哪个库？",
              header: "依赖",
              multiSelect: false,
              options: [
                { label: "A", description: "更稳" },
                { label: "B", description: "更快" },
              ],
            },
          ],
        },
      }),
      NOW + 5,
    );
    expect(views[0]).toMatchObject({
      status: "awaiting_user",
      approval: null,
      startedAt: NOW,
    });
    expect(views[0].question?.questions[0].header).toBe("依赖");
    expect(views[0].question?.questions[0].multiSelect).toBe(false);
  });

  it("approvalClosed 收敛为 cancelled 并清空审批载荷", () => {
    let views = applyAgentToolCall([], call({ phase: "started" }), NOW);
    views = applyAgentToolCall(
      views,
      call({ phase: "awaitingApproval", approval: approval() }),
      NOW + 5,
    );
    views = applyAgentToolCall(
      views,
      call({ phase: "approvalClosed", timeoutMs: 120_000 }),
      NOW + 8,
    );
    expect(views[0]).toMatchObject({
      status: "cancelled",
      approval: null,
      finishedAt: NOW + 8,
    });
  });

  it("同一 callId 只保留一条视图", () => {
    let views = applyAgentToolCall([], call({ phase: "started" }), NOW);
    views = applyAgentToolCall(views, call({ phase: "started" }), NOW + 1);
    expect(views).toHaveLength(1);
  });
});

describe("cancelPendingToolCalls 终态收敛", () => {
  it("把 running / awaiting_user 收敛为 cancelled", () => {
    let views = applyAgentToolCall([], call({ phase: "started" }), NOW);
    views = applyAgentToolCall(
      views,
      call(
        { phase: "awaitingApproval", approval: approval() },
        { callId: "call-2" },
      ),
      NOW + 1,
    );
    const settled = cancelPendingToolCalls(views, NOW + 100);
    expect(settled.map((v) => v.status)).toEqual(["cancelled", "cancelled"]);
    expect(settled[0].finishedAt).toBe(NOW + 100);
    expect(settled[1].approval).toBeNull();
  });

  it("已终态的调用不被改写", () => {
    let views = applyAgentToolCall([], call({ phase: "started" }), NOW);
    views = applyAgentToolCall(
      views,
      call({ phase: "finished", result: "ok", confirmationWaitMs: null }),
      NOW + 2,
    );
    const settled = cancelPendingToolCalls(views, NOW + 100);
    expect(settled).toBe(views);
  });

  it("无待收尾调用时返回原引用", () => {
    const views = applyAgentToolCall([], call({ phase: "started" }), NOW);
    const done = applyAgentToolCall(
      views,
      call({ phase: "finished", result: "ok", confirmationWaitMs: null }),
      NOW + 2,
    );
    expect(cancelPendingToolCalls(done, NOW + 100)).toBe(done);
  });
});

describe("hasPendingToolCalls", () => {
  it("running 与 awaiting_user 视为未收尾", () => {
    const running = applyAgentToolCall([], call({ phase: "started" }), NOW);
    expect(hasPendingToolCalls(running)).toBe(true);
    const done = applyAgentToolCall(
      running,
      call({ phase: "finished", result: "ok", confirmationWaitMs: null }),
      NOW + 2,
    );
    expect(hasPendingToolCalls(done)).toBe(false);
  });
});

describe("历史 toolCalls 映射", () => {
  it("空数组返回 undefined，避免给消息挂空工具列表", () => {
    expect(toolCallsFromHistory([])).toBeUndefined();
  });

  it("success / error / cancelled 三态转为只读视图", () => {
    const views = toolCallsFromHistory([
      {
        callId: "call-1",
        name: "read_file",
        arguments: { path: "a.txt" },
        status: "success",
        result: "ok",
      },
      {
        callId: "call-2",
        name: "write_file",
        arguments: { path: "b.txt" },
        status: "error",
        result: "denied",
      },
      {
        callId: "call-3",
        name: "shell",
        arguments: null,
        status: "cancelled",
        result: null,
      },
    ]);
    expect(views).toEqual([
      {
        callId: "call-1",
        name: "read_file",
        status: "success",
        arguments: { path: "a.txt" },
        resultText: "ok",
      },
      {
        callId: "call-2",
        name: "write_file",
        status: "error",
        arguments: { path: "b.txt" },
        resultText: "denied",
      },
      {
        callId: "call-3",
        name: "shell",
        status: "cancelled",
        arguments: undefined,
        resultText: null,
      },
    ]);
  });

  it("toChatMessage 把线格式 toolCalls 转成视图且不带原始字段", () => {
    const dto: MessageDto = {
      id: "m1",
      conversationId: "c1",
      role: "assistant",
      content: "done",
      tokens: null,
      timestamp: 1,
      parentId: null,
      isPinned: false,
      reasoning: null,
      siblingIndex: 0,
      siblingCount: null,
      providerId: null,
      modelId: null,
      toolCalls: [
        {
          callId: "call-1",
          name: "read_file",
          arguments: { path: "a.txt" },
          status: "success",
          result: "ok",
        },
      ],
    };
    const message: Message = toChatMessage(dto);
    expect(message.toolCalls).toEqual([
      {
        callId: "call-1",
        name: "read_file",
        status: "success",
        arguments: { path: "a.txt" },
        resultText: "ok",
      },
    ]);
  });

  it("toChatMessage 无工具调用时不挂 toolCalls 字段", () => {
    const dto = {
      id: "m1",
      conversationId: "c1",
      role: "user",
      content: "hi",
      tokens: null,
      timestamp: 1,
      parentId: null,
      isPinned: false,
      reasoning: null,
      siblingIndex: 0,
      siblingCount: null,
      providerId: null,
      modelId: null,
      toolCalls: [],
    } satisfies MessageDto;
    expect("toolCalls" in toChatMessage(dto)).toBe(false);
  });
});
