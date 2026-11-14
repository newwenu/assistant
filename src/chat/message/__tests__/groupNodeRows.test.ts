import { describe, it, expect } from "vitest";
import { groupNodeRows } from "../groupNodeRows";
import type { NodeGroupView } from "../groupNodeRows";
import type { Message, ToolCallView } from "../../core/chatTypes";
import { makeMessage } from "../../../test-utils/messageFixtures";

function toolView(
  callId: string,
  overrides: Partial<ToolCallView> = {},
): ToolCallView {
  return {
    callId,
    name: "shell",
    status: "success",
    arguments: { command: "npm test" },
    ...overrides,
  };
}

function msg(
  id: string,
  role: string,
  overrides: Partial<Message> = {},
): Message {
  return makeMessage(id, { role, ...overrides });
}

const base = {
  link: "agent" as const,
  phase: "settled" as const,
  currentTurnIndex: null,
};

describe("groupNodeRows 分组", () => {
  it("按 turnIndex 切组，未解析消息自成一组", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("u1", "user", { turnIndex: 0 }),
        msg("a1", "assistant", { turnIndex: 0 }),
        msg("u2", "user", { turnIndex: 1 }),
        msg("x1", "assistant", { turnIndex: null }),
      ],
    });
    expect(groups.map((g) => g.turnIndex)).toEqual([0, 1, null]);
    expect(groups[0].nodes).toHaveLength(2);
    expect(groups[2].nodes).toHaveLength(1);
  });

  it("link 只写入视图，不参与任何分支", () => {
    const input = {
      ...base,
      messages: [
        msg("u1", "user", { turnIndex: 0 }),
        msg("a1", "assistant", { turnIndex: 0 }),
      ],
    };
    const chat = groupNodeRows({ ...input, link: "chat" });
    const agent = groupNodeRows({ ...input, link: "agent" });
    expect(chat.map((g) => g.nodes.length)).toEqual(
      agent.map((g) => g.nodes.length),
    );
    expect(chat[0].link).toBe("chat");
    expect(agent[0].link).toBe("agent");
  });

  it("unresolved 组恒为 settled 且不可折叠、无尾锚点", () => {
    const groups = groupNodeRows({
      ...base,
      phase: "collecting",
      messages: [
        msg("a1", "assistant", {
          turnIndex: null,
          content: "",
          toolCalls: [toolView("c1"), toolView("c2"), toolView("c3")],
        }),
      ],
    });
    expect(groups[0].phase).toBe("settled");
    expect(groups[0].collapsible).toBe(false);
    expect(groups[0].tailAnchor).toBeNull();
  });
});

describe("groupNodeRows 相位", () => {
  it("仅当前 turn 且流式中为 collecting", () => {
    const groups = groupNodeRows({
      ...base,
      phase: "collecting",
      currentTurnIndex: 1,
      messages: [
        msg("u1", "user", { turnIndex: 0 }),
        msg("a1", "assistant", { turnIndex: 0, content: "done" }),
        msg("u2", "user", { turnIndex: 1 }),
        msg("a2", "assistant", { turnIndex: 1 }),
      ],
    });
    expect(groups.map((g) => g.phase)).toEqual(["settled", "collecting"]);
  });
});

describe("groupNodeRows 折叠判据", () => {
  it("过程行达阈值且 settled 才可折叠", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "",
          toolCalls: [toolView("c1"), toolView("c2"), toolView("c3")],
        }),
        msg("a2", "assistant", { turnIndex: 0, content: "answer" }),
      ],
    });
    expect(groups[0].collapsible).toBe(true);
    expect(groups[0].stats.steps).toBe(3);
  });

  it("1～2 步也成组（阈值 1）", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "",
          toolCalls: [toolView("c1"), toolView("c2")],
        }),
      ],
    });
    expect(groups[0].collapsible).toBe(true);
    expect(groups[0].stats.steps).toBe(2);
  });

  it("单步也成组（阈值 1）", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "",
          toolCalls: [toolView("c1")],
        }),
      ],
    });
    expect(groups[0].collapsible).toBe(true);
    expect(groups[0].nodes.map((n) => n.kind)).toEqual(["message", "tool"]);
    const bubble = groups[0].nodes[0];
    expect(bubble.kind === "message" && bubble.chromeOnly).toBe(true);
  });

  it("失败步可折叠，组头仍暴露失败数（2026-11-14 口径）", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "",
          toolCalls: [
            toolView("c1"),
            toolView("c2", { status: "error" }),
            toolView("c3"),
          ],
        }),
      ],
    });
    expect(groups[0].needsAttention).toBe(true);
    expect(groups[0].stats.failed).toBe(1);
    expect(groups[0].collapsible).toBe(true);
    expect(groups[0].nodes.map((n) => n.kind)).toEqual([
      "message",
      "tool",
      "tool",
      "tool",
    ]);
    expect(
      groups[0].nodes.map((n) => (n.kind === "tool" ? n.view.status : "")),
    ).toEqual(["", "success", "error", "success"]);
  });

  it("awaiting_user 的步切断 run，不阻止折叠也不被吞掉（安全阀）", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "",
          toolCalls: [
            toolView("c1"),
            toolView("c2", { status: "awaiting_user" }),
            toolView("c3"),
          ],
        }),
      ],
    });
    expect(groups[0].collapsible).toBe(true);
    expect(groups[0].nodes.map((n) => n.kind)).toEqual([
      "message",
      "tool",
      "tool",
      "tool",
    ]);
    const bubble = groups[0].nodes[0];
    expect(
      bubble.kind === "message" && bubble.message.toolCalls,
    ).toBeUndefined();
    const steps = groups[0].nodes.flatMap((n) =>
      n.kind === "tool" ? [n] : [],
    );
    expect(steps.map((n) => n.callId)).toEqual(["c1", "c2", "c3"]);
    expect(steps[1].run).toBeNull();
    expect(steps[0].run?.steps).toBe(1);
    expect(steps[2].run?.steps).toBe(1);
  });

  it("含边界节点时不折叠", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "",
          toolCalls: [toolView("c1"), toolView("c2"), toolView("c3")],
        }),
        msg("b1", "compact_summary", { turnIndex: 0, content: "summary" }),
      ],
    });
    expect(groups[0].collapsible).toBe(false);
  });

  it("流式中的 run 不折叠", () => {
    const groups = groupNodeRows({
      ...base,
      phase: "collecting",
      currentTurnIndex: 0,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "",
          toolCalls: [toolView("c1"), toolView("c2"), toolView("c3")],
        }),
        msg("a2", "assistant", {
          turnIndex: 0,
          status: "streaming",
          content: "…",
        }),
      ],
    });
    expect(groups[0].phase).toBe("collecting");
    expect(groups[0].collapsible).toBe(false);
  });

  it("compact 缺失时退回参数摘要，仍可折叠", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "",
          toolCalls: [
            toolView("c1", { compact: null }),
            toolView("c2", { compact: null }),
            toolView("c3", { compact: null }),
          ],
        }),
      ],
    });
    expect(groups[0].collapsible).toBe(true);
  });

  it("无摘要的步不折叠，但同 turn 其他步照常成组", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "",
          toolCalls: [
            toolView("c1"),
            toolView("c2"),
            toolView("c3", { compact: null, arguments: undefined }),
          ],
        }),
      ],
    });
    expect(groups[0].collapsible).toBe(true);
    expect(groups[0].nodes.map((n) => n.kind)).toEqual([
      "message",
      "tool",
      "tool",
      "tool",
    ]);
    const bubble = groups[0].nodes[0];
    expect(
      bubble.kind === "message" && bubble.message.toolCalls,
    ).toBeUndefined();
    const steps = groups[0].nodes.flatMap((n) =>
      n.kind === "tool" ? [n] : [],
    );
    expect(steps.map((n) => n.callId)).toEqual(["c1", "c2", "c3"]);
    expect(steps[2].run).toBeNull();
    expect(steps[0].run?.steps).toBe(2);
  });

  it("compact.head 为空串视同缺失，退回参数摘要", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "",
          toolCalls: [
            toolView("c1"),
            toolView("c2"),
            toolView("c3", {
              compact: {
                head: "  ",
                truncated: null,
                persisted: null,
                errorKind: null,
                anchor: null,
              },
            }),
          ],
        }),
      ],
    });
    expect(groups[0].collapsible).toBe(true);
  });

  it("展开后节点与折叠前逐项一致（不改数据，只改密度）", () => {
    const input = {
      ...base,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "",
          toolCalls: [toolView("c1"), toolView("c2"), toolView("c3")],
        }),
        msg("a2", "assistant", { turnIndex: 0, content: "answer" }),
      ],
    };
    const groups = groupNodeRows(input);
    const collapsedIds = groups[0].nodes
      .filter((n) => n.kind === "tool")
      .map((n) => (n.kind === "tool" ? n.callId : ""));
    expect(collapsedIds).toEqual(["c1", "c2", "c3"]);
    expect(groups[0].nodes.map((n) => n.kind)).toEqual([
      "message",
      "tool",
      "tool",
      "tool",
      "message",
    ]);
    const messageNodes = groups[0].nodes.filter((n) => n.kind === "message");
    expect(
      messageNodes.map((n) => (n.kind === "message" ? n.message.id : "")),
    ).toEqual(["a1", "a2"]);
  });
});

describe("groupNodeRows 节点投影", () => {
  it("折叠态把工具行上移为独立步行，回复行留在组外", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "",
          toolCalls: [toolView("c1"), toolView("c2"), toolView("c3")],
        }),
        msg("a2", "assistant", { turnIndex: 0, content: "answer" }),
      ],
    });
    const kinds = groups[0].nodes.map((n) => n.kind);
    expect(kinds).toEqual(["message", "tool", "tool", "tool", "message"]);
    const toolNode = groups[0].nodes[1];
    expect(toolNode.kind === "tool" && toolNode.run?.steps).toBe(3);
    const replyNode = groups[0].nodes[4];
    expect(replyNode.kind === "message" && replyNode.message.content).toBe(
      "answer",
    );
  });

  it("run 外的步渲染为独立卡片节点，气泡不再承载 toolCalls", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "partial",
          toolCalls: [toolView("c1", { status: "running" })],
        }),
      ],
    });
    expect(groups[0].nodes.map((n) => n.kind)).toEqual(["message", "tool"]);
    const node = groups[0].nodes[0];
    expect(node.kind === "message" && node.message.toolCalls).toBeUndefined();
    const step = groups[0].nodes[1];
    expect(step.kind === "tool" && step.run).toBeNull();
  });

  it("等待步排在末尾时工具步序不倒置，气泡壳留在最前挂身份行", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "",
          toolCalls: [
            toolView("c1"),
            toolView("c2"),
            toolView("wait", { status: "awaiting_user" }),
          ],
        }),
      ],
    });
    const kinds = groups[0].nodes.map((n) => n.kind);
    expect(kinds).toEqual(["message", "tool", "tool", "tool"]);
    const steps = groups[0].nodes.flatMap((n) =>
      n.kind === "tool" ? [n] : [],
    );
    expect(steps.map((n) => n.callId)).toEqual(["c1", "c2", "wait"]);
    expect(steps[2].run).toBeNull();
  });

  it("纯工具 turn 仍保留身份行与尾锚点（气泡只承载 chrome）", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "",
          toolCalls: [toolView("c1"), toolView("c2")],
        }),
      ],
    });
    const bubble = groups[0].nodes[0];
    expect(bubble.kind === "message" && bubble.chromeOnly).toBe(true);
    expect(bubble.kind === "message" && bubble.showIdentity).toBe(true);
    expect(groups[0].tailAnchor?.id).toBe("a1");
  });

  it("身份行只在 turn 起点出现", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("a1", "assistant", { turnIndex: 0, content: "one" }),
        msg("a2", "assistant", { turnIndex: 0, content: "two" }),
      ],
    });
    const identities = groups[0].nodes.map(
      (n) => n.kind === "message" && n.showIdentity,
    );
    expect(identities).toEqual([true, false]);
  });

  it("统计只来自状态与时间，不读正文", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "x",
          toolCalls: [
            toolView("c1", { startedAt: 100, finishedAt: 300 }),
            toolView("c2", { startedAt: 200, finishedAt: 500 }),
          ],
        }),
      ],
    });
    expect(groups[0].stats).toEqual({
      steps: 2,
      failed: 0,
      durationMs: 400,
    });
  });
});

describe("groupNodeRows 步段（run）", () => {
  const tools = (group: NodeGroupView) =>
    group.nodes.flatMap((node) => (node.kind === "tool" ? [node] : []));

  it("失败步不切断 run：整段工具调用共用一个 run（2026-11-14 口径）", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "",
          toolCalls: [
            toolView("c1"),
            toolView("c2"),
            toolView("c3"),
            toolView("bad", { status: "error" }),
            toolView("c4"),
            toolView("c5"),
            toolView("c6"),
          ],
        }),
        msg("a2", "assistant", { turnIndex: 0, content: "answer" }),
      ],
    });
    const steps = tools(groups[0]);
    expect(steps.map((node) => node.callId)).toEqual([
      "c1",
      "c2",
      "c3",
      "bad",
      "c4",
      "c5",
      "c6",
    ]);
    expect(new Set(steps.map((node) => node.run?.id ?? ""))).toEqual(
      new Set(["run:c1"]),
    );
    expect(steps[0].run?.steps).toBe(7);
    expect(steps[0].run?.failed).toBe(1);
    expect(groups[0].collapsible).toBe(true);
  });

  it("turn 内所有工具调用（含失败步）全部折叠成步行，气泡不残留", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "",
          toolCalls: [
            toolView("c1"),
            toolView("c2"),
            toolView("c3"),
            toolView("bad", { status: "error" }),
          ],
        }),
      ],
    });
    const bubble = groups[0].nodes.find(
      (node) => node.kind === "message" && node.message.id === "a1",
    );
    expect(
      bubble === undefined ||
        (bubble.kind === "message" && bubble.message.toolCalls === undefined),
    ).toBe(true);
    expect(tools(groups[0]).map((node) => node.callId)).toEqual([
      "c1",
      "c2",
      "c3",
      "bad",
    ]);
  });

  it("失败步计入 run 失败数，但仍与其他步同段折叠", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "",
          toolCalls: [
            toolView("c1"),
            toolView("c2"),
            toolView("c3"),
            toolView("bad", { status: "error" }),
          ],
        }),
      ],
    });
    expect(groups[0].needsAttention).toBe(true);
    expect(groups[0].collapsible).toBe(true);
    expect(tools(groups[0])[0].run?.steps).toBe(4);
    expect(tools(groups[0])[0].run?.failed).toBe(1);
  });

  it("待处理步切断 run，前后各自成段（阈值 1 时单步段也成组）", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "",
          toolCalls: [
            toolView("c1"),
            toolView("c2"),
            toolView("wait", { status: "awaiting_user" }),
            toolView("c3"),
            toolView("c4"),
            toolView("c5"),
          ],
        }),
      ],
    });
    expect(groups[0].collapsible).toBe(true);
    expect(tools(groups[0]).map((node) => node.callId)).toEqual([
      "c1",
      "c2",
      "wait",
      "c3",
      "c4",
      "c5",
    ]);
    const steps = tools(groups[0]);
    expect(steps[0].run?.steps).toBe(2);
    expect(steps[2].run).toBeNull();
    expect(steps[3].run?.steps).toBe(3);
    expect(steps[0].run?.id).not.toBe(steps[3].run?.id);
  });

  it("无摘要步切断 run，前后各自成段", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "",
          toolCalls: [
            toolView("c1"),
            toolView("c2"),
            toolView("blank", { compact: null, arguments: undefined }),
            toolView("c3"),
            toolView("c4"),
            toolView("c5"),
          ],
        }),
      ],
    });
    expect(groups[0].collapsible).toBe(true);
    expect(tools(groups[0]).map((node) => node.callId)).toEqual([
      "c1",
      "c2",
      "blank",
      "c3",
      "c4",
      "c5",
    ]);
    const steps = tools(groups[0]);
    expect(steps[0].run?.steps).toBe(2);
    expect(steps[2].run).toBeNull();
    expect(steps[3].run?.steps).toBe(3);
  });

  it("run id 与计数跨消息更新稳定", () => {
    const build = (count: number) =>
      groupNodeRows({
        ...base,
        messages: [
          msg("a1", "assistant", {
            turnIndex: 0,
            content: "",
            toolCalls: Array.from({ length: count }, (_, i) =>
              toolView(`c${i + 1}`),
            ),
          }),
        ],
      });
    expect(tools(build(3)[0])[0].run?.id).toBe(tools(build(4)[0])[0].run?.id);
    expect(tools(build(3)[0])[0].run?.steps).toBe(3);
    expect(tools(build(4)[0])[0].run?.steps).toBe(4);
  });

  it("流式增量期间不产生 run，组头计数不随全局更新跳动", () => {
    const build = (count: number) =>
      groupNodeRows({
        ...base,
        phase: "collecting",
        currentTurnIndex: 0,
        messages: [
          msg("a1", "assistant", {
            turnIndex: 0,
            content: "",
            toolCalls: Array.from({ length: count }, (_, i) =>
              toolView(`c${i + 1}`),
            ),
          }),
          msg("a2", "assistant", {
            turnIndex: 0,
            status: "streaming",
            content: "…",
          }),
        ],
      });
    for (const count of [3, 4, 5]) {
      const group = build(count)[0];
      expect(group.phase).toBe("collecting");
      expect(group.collapsible).toBe(false);
      expect(tools(group)).toHaveLength(count);
      expect(tools(group).every((node) => node.run === null)).toBe(true);
    }
  });

  it("currentTurnIndex 缺失时用流式占位行兜住 collecting", () => {
    const groups = groupNodeRows({
      ...base,
      phase: "collecting",
      currentTurnIndex: null,
      streamingMessageId: "a2",
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "",
          toolCalls: [toolView("c1"), toolView("c2"), toolView("c3")],
        }),
        msg("a2", "assistant", {
          turnIndex: 0,
          status: "streaming",
          content: "…",
        }),
      ],
    });
    expect(groups[0].phase).toBe("collecting");
    expect(groups[0].collapsible).toBe(false);
  });

  it("组头计数恒等于其下实际步数（正文行切断 run，不共用总步数）", () => {
    const messages = [
      msg("a1", "assistant", {
        turnIndex: 0,
        content: "",
        toolCalls: [toolView("c1"), toolView("c2"), toolView("c3")],
      }),
      msg("a2", "assistant", {
        turnIndex: 0,
        content: "先看看结果",
      }),
      msg("a3", "assistant", {
        turnIndex: 0,
        content: "",
        toolCalls: [toolView("c4"), toolView("c5"), toolView("c6")],
      }),
      msg("a4", "assistant", {
        turnIndex: 0,
        content: "done",
        status: "completed",
      }),
    ];
    const group = groupNodeRows({ ...base, messages })[0];
    const steps = tools(group);
    expect(steps).toHaveLength(6);
    const byRun = new Map<string, number>();
    for (const node of steps) {
      const runId = node.run?.id ?? "";
      byRun.set(runId, (byRun.get(runId) ?? 0) + 1);
    }
    expect(byRun.size).toBe(2);
    for (const node of steps) {
      expect(node.run?.steps).toBe(byRun.get(node.run?.id ?? ""));
    }
    expect(group.stats.steps).toBe(6);
  });

  it("单个 run 的步在节点序列里连续，组头只出现一次", () => {
    const messages = [
      msg("a1", "assistant", {
        turnIndex: 0,
        content: "",
        toolCalls: Array.from({ length: 12 }, (_, i) => toolView(`c${i + 1}`)),
      }),
      msg("a2", "assistant", {
        turnIndex: 0,
        content: "done",
        status: "completed",
      }),
    ];
    const group = groupNodeRows({ ...base, messages })[0];
    const toolNodes = tools(group);
    expect(toolNodes).toHaveLength(12);
    expect(new Set(toolNodes.map((node) => node.run?.id ?? "")).size).toBe(1);
    expect(toolNodes[0].run?.steps).toBe(12);
    expect(toolNodes.every((node) => node.run?.steps === 12)).toBe(true);
  });

  it("重复 callId 只计一次、只渲染一次", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "",
          toolCalls: [toolView("c1"), toolView("c2"), toolView("c3")],
        }),
        msg("a2", "assistant", {
          turnIndex: 0,
          content: "",
          toolCalls: [toolView("c3"), toolView("c4")],
        }),
      ],
    });
    const toolNodes = tools(groups[0]);
    expect(toolNodes.map((node) => node.callId)).toEqual([
      "c1",
      "c2",
      "c3",
      "c4",
    ]);
    expect(groups[0].stats.steps).toBe(4);
  });
});

describe("groupNodeRows 思考节点化", () => {
  const thoughts = (group: NodeGroupView) =>
    group.nodes.flatMap((node) => (node.kind === "reasoning" ? [node] : []));
  const tools = (group: NodeGroupView) =>
    group.nodes.flatMap((node) => (node.kind === "tool" ? [node] : []));

  it("有工具步时思考上移为独立节点，气泡不再承载 reasoning", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "",
          reasoning: "先读文件",
          toolCalls: [toolView("c1"), toolView("c2"), toolView("c3")],
        }),
        msg("a2", "assistant", { turnIndex: 0, content: "done" }),
      ],
    });
    expect(groups[0].nodes.map((node) => node.kind)).toEqual([
      "identity",
      "reasoning",
      "message",
      "tool",
      "tool",
      "tool",
      "message",
    ]);
    const thought = groups[0].nodes[1];
    expect(thought.kind === "reasoning" && thought.text).toBe("先读文件");
    const bubble = groups[0].nodes[2];
    expect(bubble.kind === "message" && bubble.message.reasoning).toBeNull();
    // 身份行上移为节点后，气泡不再重复渲染身份行，否则头像会出现两次。
    expect(bubble.kind === "message" && bubble.showIdentity).toBe(false);
  });

  it("思考与工具步共用同一 run，组头计数分离且总数恒等", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "",
          reasoning: "想一下",
          toolCalls: [toolView("c1"), toolView("c2"), toolView("c3")],
        }),
      ],
    });
    const group = groups[0];
    const thought = thoughts(group)[0];
    const steps = tools(group);
    const run = thought.run;
    expect(run).not.toBeNull();
    expect(steps.every((node) => node.run?.id === run?.id)).toBe(true);
    expect(run?.steps).toBe(3);
    expect(run?.reasoning).toBe(1);
    const sameRun = group.nodes.filter(
      (node) =>
        (node.kind === "tool" || node.kind === "reasoning") &&
        node.run?.id === run?.id,
    );
    expect(sameRun).toHaveLength((run?.steps ?? 0) + (run?.reasoning ?? 0));
  });

  it("同一消息内按「思考 → 正文 → 工具」原序输出节点", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "中间结论",
          reasoning: "先分析",
          toolCalls: [toolView("c1"), toolView("c2"), toolView("c3")],
        }),
      ],
    });
    expect(groups[0].nodes.map((node) => node.kind)).toEqual([
      "identity",
      "reasoning",
      "message",
      "tool",
      "tool",
      "tool",
    ]);
    const bubble = groups[0].nodes[2];
    expect(bubble.kind === "message" && bubble.message.content).toBe(
      "中间结论",
    );
    expect(bubble.kind === "message" && bubble.message.reasoning).toBeNull();
    expect(thoughts(groups[0])[0].run).toBeNull();
    expect(tools(groups[0])[0].run?.steps).toBe(3);
  });

  it("正文切断 run：思考随其后工具步成段，不跨气泡合段", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "",
          reasoning: "第一轮思考",
          toolCalls: [toolView("c1"), toolView("c2"), toolView("c3")],
        }),
        msg("a2", "assistant", {
          turnIndex: 0,
          content: "看看结果",
        }),
        msg("a3", "assistant", {
          turnIndex: 0,
          content: "",
          reasoning: "第二轮思考",
          toolCalls: [toolView("c4"), toolView("c5"), toolView("c6")],
        }),
      ],
    });
    const group = groups[0];
    expect(group.nodes.map((node) => node.kind)).toEqual([
      "identity",
      "reasoning",
      "message",
      "tool",
      "tool",
      "tool",
      "message",
      "reasoning",
      "tool",
      "tool",
      "tool",
    ]);
    const firstRun = thoughts(group)[0].run;
    const secondRun = thoughts(group)[1].run;
    expect(firstRun?.id).not.toBe(secondRun?.id);
    expect(firstRun?.steps).toBe(3);
    expect(secondRun?.steps).toBe(3);
  });

  it("零工具 turn 思考留在气泡内，外观与现状一致", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "答案",
          reasoning: "想一想",
        }),
      ],
    });
    expect(groups[0].nodes.map((node) => node.kind)).toEqual(["message"]);
    const bubble = groups[0].nodes[0];
    expect(bubble.kind === "message" && bubble.message.reasoning).toBe(
      "想一想",
    );
  });

  it("run 外（collecting）思考节点 run 为 null，仍独立成行", () => {
    const groups = groupNodeRows({
      ...base,
      phase: "collecting",
      currentTurnIndex: 0,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "",
          reasoning: "正在思考",
          toolCalls: [toolView("c1"), toolView("c2"), toolView("c3")],
        }),
      ],
    });
    expect(groups[0].nodes.map((node) => node.kind)).toEqual([
      "identity",
      "reasoning",
      "message",
      "tool",
      "tool",
      "tool",
    ]);
    expect(thoughts(groups[0])[0].run).toBeNull();
  });
});

describe("groupNodeRows 尾锚点", () => {
  it("优先取有正文且已 settled 的 assistant 行", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("u1", "user", { turnIndex: 0 }),
        msg("a1", "assistant", { turnIndex: 0, content: "" }),
        msg("a2", "assistant", {
          turnIndex: 0,
          content: "final",
          status: "completed",
        }),
        msg("a3", "assistant", {
          turnIndex: 0,
          content: "",
          status: "streaming",
        }),
      ],
    });
    expect(groups[0].tailAnchor?.id).toBe("a2");
  });

  it("无正文时回落到该 turn 最后一个 assistant 行", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("u1", "user", { turnIndex: 0 }),
        msg("a1", "assistant", { turnIndex: 0, content: "" }),
        msg("a2", "assistant", { turnIndex: 0, content: "" }),
      ],
    });
    expect(groups[0].tailAnchor?.id).toBe("a2");
  });

  it("turn 内无 assistant 行时不出尾", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [msg("u1", "user", { turnIndex: 0 })],
    });
    expect(groups[0].tailAnchor).toBeNull();
  });
});
