import { describe, expect, it } from "vitest";
import enUS from "@/i18n/locales/en-US.json";
import zhCN from "@/i18n/locales/zh-CN.json";
import type { I18nPath } from "@/i18n";
import type { CanvasBox } from "@/features/canvas/types";
import {
  ORCH_CARD_KINDS,
  orchCardOf,
  orchFallbackCardOf,
  orchPaletteItemsOf,
} from "../cardKinds";
import { orchEdgeLabelOf, orchEdgeOf, orchGroupLabelOf } from "../objects";
import type {
  NodeRunOverlay,
  NodeRunState,
  WorkflowEdge,
  WorkflowNode,
} from "../types";

function textAtPath(locale: object, path: string): string | undefined {
  let cursor: unknown = locale;
  for (const key of path.split(".")) {
    if (typeof cursor !== "object" || cursor === null) return undefined;
    cursor = (cursor as Record<string, unknown>)[key];
  }
  return typeof cursor === "string" ? cursor : undefined;
}

const t = (key: I18nPath): string => {
  const text = textAtPath(zhCN, key);
  if (text === undefined) throw new Error(`missing zh-CN key: ${key}`);
  return text;
};

const BOX: CanvasBox = { x: 0, y: 0, w: 300, h: 360 };

function turnNode(overrides: Partial<WorkflowNode> = {}): WorkflowNode {
  return {
    id: "n1",
    kind: "turn",
    title: "研究",
    params: {
      content: "总结这份文档",
      modeId: "chat",
      providerId: "openai",
      modelId: "gpt-4o",
    },
    groupId: null,
    ...overrides,
  };
}

function hitlNode(overrides: Partial<WorkflowNode> = {}): WorkflowNode {
  return {
    id: "n2",
    kind: "hitl",
    title: "确认",
    params: null,
    groupId: null,
    ...overrides,
  };
}

function runOf(
  state: NodeRunState,
  overrides: Partial<NodeRunOverlay> = {},
): NodeRunOverlay {
  return {
    state,
    turnId: "turn-1",
    messageId: "msg-1",
    reason: null,
    note: null,
    startedAt: null,
    ...overrides,
  };
}

function detailRowsOf(card: ReturnType<typeof orchCardOf>) {
  if (card.body.kind !== "detail") throw new Error("expected detail body");
  return card.body;
}

describe("ORCH_CARD_KINDS", () => {
  it("holds exactly the two real kinds, in fixed order", () => {
    expect(ORCH_CARD_KINDS.map((item) => item.key)).toEqual(["turn", "hitl"]);
    expect(ORCH_CARD_KINDS).toHaveLength(2);
    expect(ORCH_CARD_KINDS.map((item) => item.hasParams)).toEqual([
      true,
      false,
    ]);
  });

  it("builds a palette of only those two kinds", () => {
    const items = orchPaletteItemsOf(t);
    expect(items.map((item) => item.kind)).toEqual(["turn", "hitl"]);
    for (const item of items) {
      expect(item.badge).toBeTruthy();
      expect(item.name).toBeTruthy();
      expect(item.summary).toBeTruthy();
    }
  });
});

describe("orchCardOf — turn card (OC3 / OC5 / OC6 / OC7)", () => {
  it("keeps content, mode and provider/model in three fixed rows", () => {
    const rows = detailRowsOf(
      orchCardOf(turnNode(), null, { box: BOX, t }),
    ).rows;
    expect(orchCardOf(turnNode(), null, { box: BOX, t }).params).toEqual([
      { label: "内容", value: "总结这份文档" },
      { label: "模式", value: "chat" },
      { label: "模型", value: "openai / gpt-4o" },
    ]);
    expect(rows).toEqual([]);
  });

  it("does not truncate content in the view model", () => {
    const content = "很长的内容".repeat(20);
    const card = orchCardOf(
      turnNode({
        params: {
          content,
          modeId: null,
          providerId: null,
          modelId: null,
        },
      }),
      null,
      { box: BOX, t },
    );
    expect(card.params?.[0]?.value).toBe(content);
  });

  it("falls back to the conversation default for empty selections", () => {
    const card = orchCardOf(
      turnNode({
        params: {
          content: "",
          modeId: null,
          providerId: null,
          modelId: null,
        },
      }),
      null,
      { box: BOX, t },
    );
    expect(card.params).toEqual([
      { label: "内容", value: "—" },
      { label: "模式", value: "会话当前值" },
      { label: "模型", value: "会话当前值" },
    ]);
  });

  it("joins provider and model in all four combinations", () => {
    const cases: readonly [string | null, string | null, string][] = [
      ["openai", "gpt-4o", "openai / gpt-4o"],
      ["openai", null, "openai"],
      [null, "gpt-4o", "gpt-4o"],
      [null, null, "会话当前值"],
    ];
    for (const [providerId, modelId, expected] of cases) {
      const card = orchCardOf(
        turnNode({
          params: {
            content: "x",
            modeId: null,
            providerId,
            modelId,
          },
        }),
        null,
        { box: BOX, t },
      );
      expect(card.params?.[2]?.value).toBe(expected);
    }
  });

  it("carries no example badge, no exit and no reference", () => {
    const card = orchCardOf(turnNode(), null, { box: BOX, t });
    expect(card.exampleBadge).toBeUndefined();
    expect(card.exit).toBeUndefined();
    expect(card.reference).toBeUndefined();
  });

  it("draws two bare ports with empty type labels", () => {
    const card = orchCardOf(turnNode(), null, { box: BOX, t });
    expect(card.ports?.map((port) => port.side)).toEqual(["left", "right"]);
    for (const port of card.ports ?? []) {
      expect(port.typeLabel).toBe("");
    }
  });

  it("uses the node title, and the kind default name when empty", () => {
    expect(orchCardOf(turnNode(), null, { box: BOX, t }).title).toBe("研究");
    expect(
      orchCardOf(turnNode({ title: "   " }), null, { box: BOX, t }).title,
    ).toBe("轮次");
  });
});

describe("orchCardOf — hitl card (OC4)", () => {
  it("renders no params block", () => {
    const card = orchCardOf(hitlNode(), null, { box: BOX, t });
    expect(card.params).toBeUndefined();
    expect(card.title).toBe("确认");
    expect(card.ports).toHaveLength(2);
  });

  it("falls back to the kind default name when the title is empty", () => {
    expect(
      orchCardOf(hitlNode({ title: "" }), null, { box: BOX, t }).title,
    ).toBe("人工介入");
  });

  it("shows the not-run hint in edit state", () => {
    expect(detailRowsOf(orchCardOf(hitlNode(), null, { box: BOX, t }))).toEqual(
      {
        kind: "detail",
        label: "人工介入说明",
        rows: [],
        emptyHint: "未运行",
      },
    );
  });
});

describe("orchCardOf — run overlay (OC8 / OC9)", () => {
  const expected: readonly [NodeRunState, string, string, string][] = [
    ["pending", "default", "待发起", "待发起"],
    ["running", "active", "进行中", "turn-1"],
    ["waiting", "warning", "等待人工", "turn-1"],
    ["finished", "success", "已完成", "msg-1"],
    ["failed", "danger", "失败", "失败"],
    ["skipped", "muted", "调度阻断", "上游未走通"],
    ["cancelled", "muted", "已取消", "已取消"],
  ];

  it("maps all seven states to tone, pill and detail", () => {
    for (const [state, tone, status, detailText] of expected) {
      const card = orchCardOf(turnNode(), runOf(state), { box: BOX, t });
      expect(card.tone, state).toBe(tone);
      expect(card.status, state).toBe(status);

      const body = detailRowsOf(card);
      const texts = body.rows.map((row) => row.value);
      expect(
        body.emptyHint === undefined ? texts : [body.emptyHint],
        state,
      ).toEqual([detailText]);
    }
  });

  it("names the skipped state with its qualifier", () => {
    const card = orchCardOf(turnNode(), runOf("skipped"), { box: BOX, t });
    expect(card.status).toContain("调度阻断");
  });

  it("shows no pill and no measure without a run", () => {
    const card = orchCardOf(turnNode(), null, { box: BOX, t });
    expect(card.status).toBeUndefined();
    expect(card.measure).toBeUndefined();
  });

  it("keeps the hitl edit tone only while no run is overlaid", () => {
    expect(orchCardOf(hitlNode(), null, { box: BOX, t }).tone).toBe("warning");
    expect(orchCardOf(hitlNode(), runOf("waiting"), { box: BOX, t }).tone).toBe(
      "warning",
    );
    expect(
      orchCardOf(hitlNode(), runOf("finished"), { box: BOX, t }).tone,
    ).toBe("success");
  });

  it("adds the decision hint for a waiting hitl node", () => {
    const body = detailRowsOf(
      orchCardOf(hitlNode(), runOf("waiting", { note: "请确认" }), {
        box: BOX,
        t,
      }),
    );
    expect(body.rows).toEqual([
      { label: "备注", value: "请确认" },
      { label: "决策", value: "等待决策" },
    ]);
  });

  it("keeps reason as read-only text on failure", () => {
    const body = detailRowsOf(
      orchCardOf(turnNode(), runOf("failed", { reason: "interrupted" }), {
        box: BOX,
        t,
      }),
    );
    expect(body.rows).toEqual([{ label: "原因", value: "interrupted" }]);
  });

  it("computes the elapsed measure once from startedAt and nowMs", () => {
    const card = orchCardOf(
      turnNode(),
      runOf("running", { startedAt: 1_000 }),
      { box: BOX, t, nowMs: 2_200 },
    );
    expect(card.measure).toBe("1.2s");
  });
});

describe("orchFallbackCardOf (OC2)", () => {
  it("keeps the raw kind, drops ports and pill, and leaves data untouched", () => {
    const node: WorkflowNode = {
      id: "x1",
      kind: "delegate",
      title: "",
      params: null,
      groupId: null,
    };
    const before = JSON.stringify(node);
    const card = orchFallbackCardOf(node, { box: BOX, t });

    expect(card.kindBadge).toBe("delegate · 未知类别");
    expect(card.title).toBe("x1");
    expect(card.tone).toBe("muted");
    expect(card.ports).toBeUndefined();
    expect(card.status).toBeUndefined();
    expect(card.params).toBeUndefined();
    expect(card.exampleBadge).toBeUndefined();
    expect(card.exit).toBeUndefined();
    expect(JSON.stringify(node)).toBe(before);
  });

  it("routes unknown kinds through the fallback constructor", () => {
    const card = orchCardOf(
      { id: "x2", kind: "tool", title: "调用", params: null, groupId: null },
      null,
      { box: BOX, t },
    );
    expect(card.tone).toBe("muted");
    expect(card.kindBadge).toBe("tool · 未知类别");
    expect(card.title).toBe("调用");
  });
});

describe("non-card objects (OC13)", () => {
  const from: CanvasBox = { x: 0, y: 0, w: 300, h: 360 };
  const to: CanvasBox = { x: 400, y: 0, w: 300, h: 360 };

  function edgeOf(
    condition: WorkflowEdge["condition"],
    id = "e1",
  ): WorkflowEdge {
    return { id, from: "n1", to: "n2", condition };
  }

  it("maps endpoints directly and never emits an outer track", () => {
    const horizontal = orchEdgeOf(edgeOf("always"), from, to);
    expect(horizontal).toEqual({
      id: "e1",
      from: "n1",
      to: "n2",
      route: "horizontal",
      phase: "idle",
    });

    const vertical = orchEdgeOf(
      edgeOf("always"),
      { x: 0, y: 0, w: 300, h: 360 },
      { x: 0, y: 500, w: 300, h: 360 },
    );
    expect(vertical.route).toBe("vertical");
  });

  it("generates condition labels only for onSuccess and onFailure", () => {
    expect(orchEdgeLabelOf(edgeOf("always"), from, to, t)).toBeNull();
    for (const condition of ["onSuccess", "onFailure"] as const) {
      const label = orchEdgeLabelOf(edgeOf(condition), from, to, t);
      expect(label?.tone).toBe("muted");
      expect(label?.text).toBeTruthy();
      expect(label?.box.x).toBeGreaterThan(0);
    }
  });

  it("emits a single muted label for a group, boxed 8px around members", () => {
    const label = orchGroupLabelOf(
      { id: "g1", title: "一组", nodeIds: ["n1"] },
      [from, to],
    );
    expect(label).toEqual({
      id: "group-g1",
      box: { x: -8, y: -8, w: 716, h: 376 },
      text: "一组",
      tone: "muted",
    });
    expect(label?.frame).toBeUndefined();
  });

  it("draws no group label for an empty group", () => {
    expect(
      orchGroupLabelOf({ id: "g2", title: "空组", nodeIds: [] }, []),
    ).toBeNull();
  });

  it("opts into the dashed frame only when asked", () => {
    const label = orchGroupLabelOf(
      { id: "g3", title: "一组", nodeIds: ["n1"] },
      [from],
      { frame: true },
    );
    expect(label?.frame).toBe(true);
  });
});

describe("i18n parity for orch keys", () => {
  const KEYS: readonly I18nPath[] = [
    "orch.card.kind.turn",
    "orch.card.kind.hitl",
    "orch.card.kind.unknown",
    "orch.card.name.turn",
    "orch.card.name.hitl",
    "orch.card.summary.turn",
    "orch.card.summary.hitl",
    "orch.card.param.content",
    "orch.card.param.mode",
    "orch.card.param.model",
    "orch.card.value.inherited",
    "orch.card.value.decision_pending",
    "orch.card.row.turn",
    "orch.card.row.note",
    "orch.card.row.result",
    "orch.card.row.reason",
    "orch.card.row.decision",
    "orch.card.empty.idle",
    "orch.card.empty.pending",
    "orch.card.empty.skipped",
    "orch.card.empty.cancelled",
    "orch.card.empty.unknown_kind",
    "orch.card.hitl.section",
    "orch.node.state.pending",
    "orch.node.state.running",
    "orch.node.state.waiting",
    "orch.node.state.finished",
    "orch.node.state.failed",
    "orch.node.state.skipped",
    "orch.node.state.cancelled",
    "orch.edge.cond.onSuccess",
    "orch.edge.cond.onFailure",
  ];

  it("has every key in both locales", () => {
    for (const key of KEYS) {
      expect(textAtPath(zhCN, key), `zh-CN ${key}`).toBeTruthy();
      expect(textAtPath(enUS, key), `en-US ${key}`).toBeTruthy();
    }
  });
});
