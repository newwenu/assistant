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

  it("不足阈值不折叠", () => {
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
    expect(groups[0].collapsible).toBe(false);
  });

  it("失败步切断 run，残余段不足阈值时不折叠（安全阀）", () => {
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
    expect(groups[0].collapsible).toBe(false);
    expect(groups[0].stats.failed).toBe(1);
    // 失败步留在气泡里完整显示，不能被折叠吞掉
    const bubble = groups[0].nodes[0];
    expect(bubble.kind === "message" && bubble.message.toolCalls).toHaveLength(
      3,
    );
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

  it("compact 与参数都缺失时不折叠（宁可高，不可空）", () => {
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
    expect(groups[0].collapsible).toBe(false);
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
      "tool",
      "tool",
      "tool",
      "message",
    ]);
    const messageNodes = groups[0].nodes.filter((n) => n.kind === "message");
    expect(
      messageNodes.map((n) => (n.kind === "message" ? n.message.id : "")),
    ).toEqual(["a2"]);
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
    expect(kinds).toEqual(["tool", "tool", "tool", "message"]);
    const toolNode = groups[0].nodes[0];
    expect(toolNode.kind === "tool" && toolNode.run.steps).toBe(3);
    const replyNode = groups[0].nodes[3];
    expect(replyNode.kind === "message" && replyNode.message.content).toBe(
      "answer",
    );
  });

  it("非折叠态工具留在气泡内，不产生步行", () => {
    const groups = groupNodeRows({
      ...base,
      messages: [
        msg("a1", "assistant", {
          turnIndex: 0,
          content: "partial",
          toolCalls: [toolView("c1")],
        }),
      ],
    });
    expect(groups[0].nodes.map((n) => n.kind)).toEqual(["message"]);
    const node = groups[0].nodes[0];
    expect(node.kind === "message" && node.message.toolCalls).toHaveLength(1);
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

  it("失败步切断 run，前后成功步各自折叠、各自计数", () => {
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
      "c4",
      "c5",
      "c6",
    ]);
    expect(steps[0].run.id).toBe("run:c1");
    expect(steps[0].run.steps).toBe(3);
    expect(steps[3].run.id).toBe("run:c4");
    expect(steps[3].run.steps).toBe(3);
    expect(groups[0].collapsible).toBe(true);
  });

  it("失败步留在气泡内，不被同条消息的折叠步吞掉", () => {
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
      bubble?.kind === "message" &&
        bubble.message.toolCalls?.map((view) => view.callId),
    ).toEqual(["bad"]);
    expect(tools(groups[0]).map((node) => node.callId)).toEqual([
      "c1",
      "c2",
      "c3",
    ]);
  });

  it("单节点错误不阻止其他节点折叠（不再整组失效）", () => {
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
    expect(tools(groups[0])[0].run.steps).toBe(3);
  });

  it("待处理步同样切断 run", () => {
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
      "c3",
      "c4",
      "c5",
    ]);
    expect(tools(groups[0])[0].run.steps).toBe(3);
  });

  it("无摘要步切断 run", () => {
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
      "c3",
      "c4",
      "c5",
    ]);
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
    expect(tools(build(3)[0])[0].run.id).toBe(tools(build(4)[0])[0].run.id);
    expect(tools(build(3)[0])[0].run.steps).toBe(3);
    expect(tools(build(4)[0])[0].run.steps).toBe(4);
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
      expect(tools(group)).toHaveLength(0);
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
      byRun.set(node.run.id, (byRun.get(node.run.id) ?? 0) + 1);
    }
    expect(byRun.size).toBe(2);
    for (const node of steps) {
      expect(node.run.steps).toBe(byRun.get(node.run.id));
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
    expect(new Set(toolNodes.map((node) => node.run.id)).size).toBe(1);
    expect(toolNodes[0].run.steps).toBe(12);
    expect(toolNodes.every((node) => node.run.steps === 12)).toBe(true);
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
