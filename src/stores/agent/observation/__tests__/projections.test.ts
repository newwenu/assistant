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
import type { TurnObservation } from "../types";

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
    expect(projectGraph(obs).activeEdge).toEqual({
      from: "ladder",
      to: "model",
      kind: "forward",
    });
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
