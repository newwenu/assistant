import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AgentNodeEventDto } from "../../../../types/protocol.contract";
import {
  applyAssumedCell,
  applyNodeEvent,
  createObservation,
} from "../reducer";
import { projectGraph } from "../projections/graph";
import { projectTimeline } from "../projections/timeline";
import { projectTurnSummary } from "../projections/summary";
import { subscribeProjections, type ProjectionsListener } from "../schedule";
import { useObservationStore } from "../store";
import { TEMPLATE_EDGES, templateOrderOf } from "../template";
import type {
  AgentNodeId,
  TurnObservation,
  TurnObservationStatus,
} from "../types";

let seq = 0;

function ev(
  override: Partial<AgentNodeEventDto> &
    Pick<AgentNodeEventDto, "node" | "state">,
): AgentNodeEventDto {
  seq += 1;
  return {
    conversationId: "conv-1",
    turnId: "conv-1:1",
    seq,
    iteration: 1,
    transition: true,
    atMs: seq * 10,
    agentId: "root",
    parentTurnId: null,
    detail: null,
    ...override,
  };
}

function fresh(): TurnObservation {
  return createObservation("conv-1", "conv-1:1", 0);
}

beforeEach(() => {
  seq = 0;
  useObservationStore.setState({ observations: new Map() });
});

describe("projectGraph — 12 格恒定输出", () => {
  it("无任何格子时全部为 pending + derived，且圈数为 1", () => {
    const view = projectGraph(fresh());
    expect(view.nodes).toHaveLength(12);
    expect(view.iteration).toBe(1);
    for (const node of view.nodes) {
      expect(node.state).toBe("pending");
      expect(node.source).toBe("derived");
    }
    expect(view.activeEdge).toBeNull();
  });

  it("迭代外节点的 iteration 恒为 0，迭代内节点带圈数", () => {
    let obs = fresh();
    obs = applyNodeEvent(
      obs,
      ev({ node: "model", state: "running", iteration: 2 }),
    );
    const view = projectGraph(obs);
    expect(view.iteration).toBe(2);
    expect(view.nodes.find((node) => node.id === "admit")?.iteration).toBe(0);
    expect(view.nodes.find((node) => node.id === "model")?.iteration).toBe(2);
    expect(view.nodes.find((node) => node.id === "model")?.state).toBe(
      "running",
    );
    expect(view.nodes.find((node) => node.id === "model")?.source).toBe(
      "observed",
    );
    expect(view.nodes.find((node) => node.id === "model")?.unfinished).toBe(
      true,
    );
  });

  it("活动边指向最靠后的进行中格子", () => {
    let obs = fresh();
    obs = applyNodeEvent(
      obs,
      ev({ node: "model", state: "running", iteration: 2 }),
    );
    const view = projectGraph(obs);
    expect(view.activeEdge).toEqual({
      from: "ladder",
      to: "model",
      kind: "forward",
      phase: "active",
    });
    expect(view.activeEdge).toBe(
      view.edges.find((edge) => edge.phase === "active") ?? null,
    );
    expect(view.edges.filter((edge) => edge.phase === "active")).toHaveLength(
      1,
    );
  });

  it("第二轮及以后的准入由回边 A 进入", () => {
    let obs = fresh();
    obs = applyNodeEvent(
      obs,
      ev({ node: "after-iteration", state: "finished", iteration: 2 }),
    );
    obs = applyNodeEvent(
      obs,
      ev({ node: "admit", state: "running", iteration: 0 }),
    );
    expect(projectGraph(obs).activeEdge).toEqual({
      from: "after-iteration",
      to: "admit",
      kind: "loop",
      phase: "active",
    });
  });
});

describe("TEMPLATE_EDGES — 边集恒定 12 条", () => {
  it("11 条正向边逐条相邻，外加 1 条回边 ⑫→①", () => {
    expect(TEMPLATE_EDGES).toHaveLength(12);
    const forward = TEMPLATE_EDGES.filter((edge) => edge.kind === "forward");
    expect(forward).toHaveLength(11);
    for (const edge of forward) {
      expect(templateOrderOf(edge.to)).toBe(
        (templateOrderOf(edge.from) ?? Number.NaN) + 1,
      );
    }
    expect(TEMPLATE_EDGES.filter((edge) => edge.kind === "loop")).toEqual([
      { from: "after-iteration", to: "admit", kind: "loop" },
    ]);
  });
});

describe("projectGraph — 边相位", () => {
  type GraphView = ReturnType<typeof projectGraph>;

  function edgeOf(view: GraphView, from: AgentNodeId, to: AgentNodeId) {
    const edge = view.edges.find(
      (item) => item.from === from && item.to === to,
    );
    if (edge === undefined) throw new Error(`缺边 ${from}→${to}`);
    return edge;
  }

  it("正向边：from 未触达 untaken、from 触达 idle、to 触达 passed", () => {
    expect(edgeOf(projectGraph(fresh()), "admit", "prepare").phase).toBe(
      "untaken",
    );

    const idle = applyNodeEvent(
      fresh(),
      ev({ node: "admit", state: "running", iteration: 0 }),
    );
    expect(edgeOf(projectGraph(idle), "admit", "prepare").phase).toBe("idle");

    const passed = applyNodeEvent(
      fresh(),
      ev({ node: "brake", state: "finished", iteration: 1 }),
    );
    expect(edgeOf(projectGraph(passed), "prepare", "brake").phase).toBe(
      "passed",
    );
  });

  it("正向边：to 被标 skipped 时保持 untaken，不画成 passed", () => {
    const obs = applyNodeEvent(
      fresh(),
      ev({
        node: "branch",
        state: "finished",
        iteration: 1,
        detail: {
          kind: "branch",
          hasToolCalls: false,
          wrapUp: false,
          toolCallCount: 0,
        },
      }),
    );
    const view = projectGraph(obs);
    expect(view.nodes.find((node) => node.id === "budget-split")?.state).toBe(
      "skipped",
    );
    expect(edgeOf(view, "branch", "budget-split").phase).toBe("untaken");
  });

  it("回边：⑫ 未触达 untaken、⑫ 触达 idle、圈数 ≥ 2 passed", () => {
    expect(
      edgeOf(projectGraph(fresh()), "after-iteration", "admit").phase,
    ).toBe("untaken");

    const idle = applyNodeEvent(
      fresh(),
      ev({ node: "after-iteration", state: "finished", iteration: 1 }),
    );
    expect(edgeOf(projectGraph(idle), "after-iteration", "admit").phase).toBe(
      "idle",
    );

    const passed = applyNodeEvent(
      fresh(),
      ev({ node: "after-iteration", state: "finished", iteration: 2 }),
    );
    expect(edgeOf(projectGraph(passed), "after-iteration", "admit").phase).toBe(
      "passed",
    );
  });

  it("无进行中格子时不存在 active 相位，activeEdge 为 null", () => {
    const view = projectGraph(fresh());
    expect(view.edges.every((edge) => edge.phase !== "active")).toBe(true);
    expect(view.activeEdge).toBeNull();
  });
});

describe("projectGraph — 出口标注", () => {
  function withStatus(
    obs: TurnObservation,
    status: TurnObservationStatus,
  ): TurnObservation {
    return { ...obs, status };
  }

  it("running 无出口；completed / wrap_up 的出口为 ⑦ branch", () => {
    expect(projectGraph(fresh()).exit).toBeNull();

    const completed = applyNodeEvent(
      fresh(),
      ev({ node: "branch", state: "finished", iteration: 1 }),
    );
    expect(projectGraph(withStatus(completed, "completed")).exit).toEqual({
      node: "branch",
      status: "completed",
    });
    expect(projectGraph(withStatus(completed, "wrap_up")).exit).toEqual({
      node: "branch",
      status: "wrap_up",
    });
  });

  it("failed 取最靠后的已触达候选（⑤ model 优先于 ④ ladder）", () => {
    const obs = withStatus(
      applyNodeEvent(
        fresh(),
        ev({ node: "model", state: "error", iteration: 1 }),
      ),
      "failed",
    );
    expect(projectGraph(obs).exit).toEqual({ node: "model", status: "failed" });
  });

  it("cancelled 取 ⑨ pipeline，stopped 取 ⑩ after-tool", () => {
    const cancelled = withStatus(
      applyNodeEvent(
        fresh(),
        ev({ node: "pipeline", state: "running", iteration: 1 }),
      ),
      "cancelled",
    );
    expect(projectGraph(cancelled).exit).toEqual({
      node: "pipeline",
      status: "cancelled",
    });

    const stopped = withStatus(
      applyNodeEvent(
        fresh(),
        ev({ node: "after-tool", state: "finished", iteration: 1 }),
      ),
      "stopped",
    );
    expect(projectGraph(stopped).exit).toEqual({
      node: "after-tool",
      status: "stopped",
    });
  });

  it("halted_by_budget 出口为 ③ brake；候选全 pending 时取最小 order", () => {
    const halted = withStatus(
      applyNodeEvent(
        fresh(),
        ev({ node: "brake", state: "finished", iteration: 1 }),
      ),
      "halted_by_budget",
    );
    expect(projectGraph(halted).exit).toEqual({
      node: "brake",
      status: "halted_by_budget",
    });

    expect(projectGraph(withStatus(fresh(), "stopped")).exit).toEqual({
      node: "brake",
      status: "stopped",
    });
    expect(projectGraph(withStatus(fresh(), "completed")).exit).toEqual({
      node: "branch",
      status: "completed",
    });
  });
});

describe("projectTimeline — 按 seq 排序且保留 transition", () => {
  it("排序不丢事实，过程事实的 transition 原样保留", () => {
    let obs = fresh();
    obs = applyNodeEvent(
      obs,
      ev({
        node: "pipeline",
        state: "running",
        iteration: 1,
        transition: false,
      }),
    );
    obs = applyNodeEvent(
      obs,
      ev({ node: "pipeline", state: "running", iteration: 1 }),
    );
    const facts = projectTimeline(obs);
    expect(facts).toHaveLength(2);
    expect(facts[0].seq).toBeLessThan(facts[1].seq);
    expect(facts[0].transition).toBe(false);
    expect(facts[1].transition).toBe(true);
  });
});

describe("projectTurnSummary — 跳过 assumed 格子", () => {
  it("耗时只计已闭合的 observed 段，assumed 与进行中的格子被排除", () => {
    let obs = fresh();
    obs = applyNodeEvent(
      obs,
      ev({ node: "ladder", state: "running", iteration: 1, atMs: 0 }),
    );
    obs = applyNodeEvent(
      obs,
      ev({ node: "ladder", state: "finished", iteration: 1, atMs: 10 }),
    );
    obs = applyNodeEvent(
      obs,
      ev({ node: "model", state: "running", iteration: 1, atMs: 10 }),
    );
    obs = applyNodeEvent(
      obs,
      ev({ node: "model", state: "finished", iteration: 1, atMs: 60 }),
    );
    obs = applyNodeEvent(
      obs,
      ev({ node: "after-tool", state: "running", iteration: 1, atMs: 100 }),
    );
    obs = applyAssumedCell(obs, "pipeline", 1, "waiting", 200);

    const view = projectTurnSummary(obs);
    expect(view.durations.map((item) => item.node)).toEqual([
      "ladder",
      "model",
    ]);
    expect(view.durations[1].durationMs).toBe(50);
    expect(view.measuredMs).toBe(60);
    expect(view.assumedCells).toBe(1);
    expect(view.durations.some((item) => item.node === "after-tool")).toBe(
      false,
    );
  });

  it("running ⇄ waiting 不拆段，预算余量取最近一次 brake", () => {
    let obs = fresh();
    obs = applyNodeEvent(
      obs,
      ev({ node: "pipeline", state: "running", iteration: 1, atMs: 10 }),
    );
    obs = applyNodeEvent(
      obs,
      ev({ node: "pipeline", state: "waiting", iteration: 1, atMs: 20 }),
    );
    obs = applyNodeEvent(
      obs,
      ev({ node: "pipeline", state: "running", iteration: 1, atMs: 40 }),
    );
    obs = applyNodeEvent(
      obs,
      ev({ node: "pipeline", state: "finished", iteration: 1, atMs: 60 }),
    );
    obs = applyNodeEvent(
      obs,
      ev({
        node: "brake",
        state: "finished",
        iteration: 1,
        detail: {
          kind: "brake",
          verdict: "continue",
          hook: null,
          budget: {
            remainingIterations: 3,
            remainingToolCalls: null,
            remainingWallClockMs: null,
          },
        },
      }),
    );

    const view = projectTurnSummary(obs);
    expect(
      view.durations.find((item) => item.node === "pipeline")?.durationMs,
    ).toBe(50);
    expect(view.budget?.remainingIterations).toBe(3);
    expect(view.budget?.remainingToolCalls).toBeNull();
  });
});

describe("subscribeProjections — 单点调度", () => {
  it("同一帧内多条事件只重算一次，且事实不丢", async () => {
    const listener = vi.fn<ProjectionsListener>();
    const unsubscribe = subscribeProjections("conv-1", "conv-1:1", listener);
    expect(listener).toHaveBeenCalledTimes(1);

    const store = useObservationStore.getState();
    store.applyNodeEvent(
      ev({ node: "ladder", state: "running", iteration: 1 }),
    );
    store.applyNodeEvent(
      ev({ node: "ladder", state: "finished", iteration: 1 }),
    );
    store.applyNodeEvent(ev({ node: "model", state: "running", iteration: 1 }));

    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(listener).toHaveBeenCalledTimes(2);
    const projections = listener.mock.calls[1][0];
    expect(projections).not.toBeNull();
    if (projections === null) throw new Error("投影未回调");
    expect(projections.timeline).toHaveLength(3);
    expect(
      projections.graph.nodes.find((node) => node.id === "model")?.state,
    ).toBe("running");

    unsubscribe();
  });

  it("无真实变化的折叠不触发重算（O11 引用相等抑制）", async () => {
    const store = useObservationStore.getState();
    store.applyNodeEvent(
      ev({ node: "ladder", state: "running", iteration: 1 }),
    );
    store.applyNodeEvent(
      ev({ node: "ladder", state: "finished", iteration: 1 }),
    );
    store.applySessionState("conv-1", "conv-1:1", "idle");
    const settled = useObservationStore
      .getState()
      .getObservation("conv-1", "conv-1:1")!;

    const listener = vi.fn();
    const unsubscribe = subscribeProjections("conv-1", "conv-1:1", listener);
    expect(listener).toHaveBeenCalledTimes(1);

    store.applyNodeEvent(
      ev({ node: "ladder", state: "running", iteration: 1 }),
    );
    store.applySessionState("conv-1", "conv-1:1", "idle");

    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(
      useObservationStore.getState().getObservation("conv-1", "conv-1:1"),
    ).toBe(settled);
    expect(listener).toHaveBeenCalledTimes(1);

    unsubscribe();
  });
});
