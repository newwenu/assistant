import { describe, it, expect } from "vitest";
import type {
  AgentToolCall,
  AgentToolCallPayload,
  Message,
} from "../chatTypes";
import type { MessageDto } from "../../../types/protocol.contract";
import type {
  ToolErrorKindDto,
  ToolResultCompactDto,
} from "../../../types/protocol.contract";
import {
  makeMessage,
  makeMessageDto,
} from "../../../test-utils/messageFixtures";
import {
  applyAgentToolCall,
  applyPendingToolCalls,
  cancelPendingToolCalls,
  findToolCallOwner,
  hasPendingToolCalls,
  toChatMessage,
  toolCallsFromHistory,
} from "../toolCallView";

const NOW = 1_700_000_000_000;

type PayloadInput =
  | { phase: "started"; argsPreview?: string | null }
  | {
      phase: "finished";
      result: string;
      confirmationWaitMs?: number | null;
      compact?: ToolResultCompactDto | null;
    }
  | {
      phase: "failed";
      error: string;
      errorKind: ToolErrorKindDto;
      confirmationWaitMs?: number | null;
      compact?: ToolResultCompactDto | null;
    }
  | Exclude<AgentToolCallPayload, { phase: "started" | "finished" | "failed" }>;

/** 终态载荷的 `compact` 与 `argsPreview` 键恒在，测试按最小写法补齐 `null`。 */
function normalizePayload(payload: PayloadInput): AgentToolCallPayload {
  switch (payload.phase) {
    case "started":
      return { phase: "started", argsPreview: payload.argsPreview ?? null };
    case "finished":
      return {
        phase: "finished",
        result: payload.result,
        confirmationWaitMs: payload.confirmationWaitMs ?? null,
        compact: payload.compact ?? null,
      };
    case "failed":
      return {
        phase: "failed",
        error: payload.error,
        errorKind: payload.errorKind,
        confirmationWaitMs: payload.confirmationWaitMs ?? null,
        compact: payload.compact ?? null,
      };
    default:
      return payload;
  }
}

function call(
  payload: PayloadInput,
  overrides: Partial<AgentToolCall> = {},
): AgentToolCall {
  return {
    conversationId: "conv-1",
    turnId: "turn-1",
    iteration: 1,
    callId: "call-1",
    name: "shell",
    payload: normalizePayload(payload),
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

  it("首个 argsDelta 直接建卡为 preparing（S9）", () => {
    const views = applyAgentToolCall(
      [],
      call({ phase: "argsDelta", delta: '{"pa' }),
      NOW,
    );
    expect(views).toHaveLength(1);
    expect(views[0]).toMatchObject({
      callId: "call-1",
      name: "shell",
      status: "preparing",
      startedAt: NOW,
      argsText: '{"pa',
    });
  });

  it("身份声明（空串）即出行（S11）", () => {
    const views = applyAgentToolCall(
      [],
      call({ phase: "argsDelta", delta: "" }),
      NOW,
    );
    expect(views[0]).toMatchObject({
      status: "preparing",
      startedAt: NOW,
      argsText: "",
    });
  });

  it("Started 覆写 startedAt 且不清空 argsText（S10）", () => {
    let views = applyAgentToolCall(
      [],
      call({ phase: "argsDelta", delta: '{"pa' }),
      NOW,
    );
    views = applyAgentToolCall(
      views,
      call({ phase: "started", argsPreview: "shell ls" }),
      NOW + 8_000,
    );
    expect(views[0]).toMatchObject({
      status: "running",
      startedAt: NOW + 8_000,
      argsText: '{"pa',
      argsPreview: "shell ls",
    });
  });

  it("审批通过后的 Started 把 awaiting_user 推回 running 并清掉审批对象（I74 §3.2）", () => {
    let views = applyAgentToolCall(
      [],
      call({ phase: "awaitingApproval", approval: approval() }),
      NOW,
    );
    expect(views[0]).toMatchObject({
      status: "awaiting_user",
      startedAt: NOW,
      arguments: { command: ["ls", "-la"] },
    });

    views = applyAgentToolCall(
      views,
      call({ phase: "started", argsPreview: "shell ls" }),
      NOW + 30_000,
    );
    expect(views[0]).toMatchObject({
      status: "running",
      startedAt: NOW + 30_000,
      argsText: "",
      argsPreview: "shell ls",
      approval: null,
    });
  });

  it("Started 之后迟到的 argsDelta 不降级为 preparing", () => {
    let views = applyAgentToolCall([], call({ phase: "started" }), NOW);
    views = applyAgentToolCall(
      views,
      call({ phase: "argsDelta", delta: "tail" }),
      NOW + 1,
    );
    expect(views[0]).toMatchObject({
      status: "running",
      startedAt: NOW,
      argsText: "tail",
    });
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

  it("failed + cancelled 收敛为 cancelled 而非 error（S17）", () => {
    const views = applyAgentToolCall(
      [],
      call({ phase: "argsDelta", delta: "" }),
      NOW,
    );
    const failed = applyAgentToolCall(
      views,
      call({ phase: "failed", error: "已取消", errorKind: "cancelled" }),
      NOW + 3,
    );
    expect(failed[0]).toMatchObject({
      status: "cancelled",
      errorKind: "cancelled",
      finishedAt: NOW + 3,
    });
  });

  it("failed + 非 cancelled 仍为 error（S18）", () => {
    const views = applyAgentToolCall([], call({ phase: "started" }), NOW);
    const failed = applyAgentToolCall(
      views,
      call({ phase: "failed", error: "超时", errorKind: "timeout" }),
      NOW + 3,
    );
    expect(failed[0]).toMatchObject({ status: "error", errorKind: "timeout" });
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

  it("把 preparing 收敛为 cancelled（S12）", () => {
    const views = applyAgentToolCall(
      [],
      call({ phase: "argsDelta", delta: '{"pa' }),
      NOW,
    );
    const settled = cancelPendingToolCalls(views, NOW + 4_000);
    expect(settled[0]).toMatchObject({
      status: "cancelled",
      finishedAt: NOW + 4_000,
    });
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

  it("preparing 同样视为未收尾", () => {
    const preparing = applyAgentToolCall(
      [],
      call({ phase: "argsDelta", delta: "" }),
      NOW,
    );
    expect(hasPendingToolCalls(preparing)).toBe(true);
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
        compact: null,
      },
      {
        callId: "call-2",
        name: "write_file",
        arguments: { path: "b.txt" },
        status: "error",
        result: "denied",
        compact: null,
      },
      {
        callId: "call-3",
        name: "shell",
        arguments: null,
        status: "cancelled",
        result: null,
        compact: null,
      },
    ]);
    expect(views).toEqual([
      {
        callId: "call-1",
        name: "read_file",
        status: "success",
        arguments: { path: "a.txt" },
        resultText: "ok",
        compact: null,
      },
      {
        callId: "call-2",
        name: "write_file",
        status: "error",
        arguments: { path: "b.txt" },
        resultText: "denied",
        compact: null,
      },
      {
        callId: "call-3",
        name: "shell",
        status: "cancelled",
        arguments: undefined,
        resultText: null,
        compact: null,
      },
    ]);
  });

  it("toChatMessage 把线格式 toolCalls 转成视图且不带原始字段", () => {
    const dto: MessageDto = makeMessageDto("m1", {
      conversationId: "c1",
      role: "assistant",
      content: "done",
      timestamp: 1,
      toolCalls: [
        {
          callId: "call-1",
          name: "read_file",
          arguments: { path: "a.txt" },
          status: "success",
          result: "ok",
          compact: null,
        },
      ],
    });
    const message: Message = toChatMessage(dto);
    expect(message.toolCalls).toEqual([
      {
        callId: "call-1",
        name: "read_file",
        status: "success",
        arguments: { path: "a.txt" },
        resultText: "ok",
        compact: null,
      },
    ]);
  });

  it("toChatMessage 无工具调用时不挂 toolCalls 字段", () => {
    const dto = makeMessageDto("m1", {
      conversationId: "c1",
      role: "user",
      content: "hi",
      timestamp: 1,
    });
    expect("toolCalls" in toChatMessage(dto)).toBe(false);
  });
});

describe("补拉快照回挂（list_pending_questions）", () => {
  function historyMessage(id: string, callIds: string[]): Message {
    return makeMessage(id, {
      role: "assistant",
      timestamp: 1,
      toolCalls: callIds.map((callId) => ({
        callId,
        name: "ask_user",
        status: "missing" as const,
        resultText: null,
      })),
    });
  }

  function pendingQuestion(callId: string, iteration = 1): AgentToolCall {
    return call(
      {
        phase: "awaitingAnswer",
        question: {
          conversationId: "conv-1",
          turnId: "turn-1",
          callId,
          timeoutMs: 60_000,
          questions: [
            {
              question: "继续吗？",
              header: "确认",
              multiSelect: false,
              options: [{ label: "继续", description: "" }],
            },
          ],
        },
      },
      { callId, iteration },
    );
  }

  it("把等待期的 missing 残卡改写为可作答的 awaiting_user", () => {
    const messages = [historyMessage("msg-1", ["call-1"])];
    const next = applyPendingToolCalls(
      messages,
      [pendingQuestion("call-1")],
      NOW,
    );
    expect(next[0].toolCalls?.[0]).toMatchObject({
      status: "awaiting_user",
      startedAt: NOW,
    });
    expect(next[0].toolCalls?.[0].question?.questions[0].header).toBe("确认");
    expect(next[0].toolCalls?.[0].question?.timeoutMs).toBe(60_000);
  });

  it("只回挂命中的 callId，其余消息保持原引用", () => {
    const messages = [
      historyMessage("msg-1", ["call-1"]),
      historyMessage("msg-2", ["call-2"]),
    ];
    const next = applyPendingToolCalls(
      messages,
      [pendingQuestion("call-1")],
      NOW,
    );
    expect(next[0].toolCalls?.[0].status).toBe("awaiting_user");
    expect(next[1]).toBe(messages[1]);
  });

  it("快照为空或无命中时返回原引用", () => {
    const messages = [historyMessage("msg-1", ["call-1"])];
    expect(applyPendingToolCalls(messages, [], NOW)).toBe(messages);
    expect(
      applyPendingToolCalls(messages, [pendingQuestion("call-9")], NOW),
    ).toBe(messages);
  });

  it("findToolCallOwner 按 callId 定位发起行", () => {
    const messages = [
      historyMessage("msg-1", ["call-1"]),
      historyMessage("msg-2", ["call-2"]),
    ];
    expect(findToolCallOwner(messages, "call-2")?.id).toBe("msg-2");
    expect(findToolCallOwner(messages, "call-9")).toBeUndefined();
  });
});
