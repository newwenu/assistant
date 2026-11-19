import { beforeEach, describe, expect, it } from "vitest";
import type {
  AgentNodeEventDto,
  AgentQuestionRequestDto,
  AgentToolCallDto,
  ToolApprovalRequestDto,
} from "../../../../types/protocol.contract";
import {
  applyAssumedCell,
  applyDeltaSeq,
  applyNodeEvent,
  applySessionState,
  applyToolCall,
  createObservation,
  TIMELINE_LIMIT,
} from "../reducer";
import { restoreObservation } from "../restore";
import type { TurnObservation } from "../types";

let seq = 0;

type EventOverride = Partial<AgentNodeEventDto> &
  Pick<AgentNodeEventDto, "node" | "state">;

function ev(override: EventOverride): AgentNodeEventDto {
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

/**
 * 未知取值构造：`node` / `state` 故意给闭集外的字符串（I55 §4.8 第 3 条）。
 * 入参不取 `Partial<AgentNodeEventDto>`，否则 `string & AgentNodeDto` 会把
 * 交叉类型收窄回闭集，未知取值根本传不进来。
 */
function unknownEv(override: {
  node: string;
  state: string;
  iteration?: number;
  atMs?: number;
  transition?: boolean;
  detail?: AgentNodeEventDto["detail"];
}): AgentNodeEventDto {
  seq += 1;
  const event = {
    conversationId: "conv-1",
    turnId: "conv-1:1",
    seq,
    iteration: override.iteration ?? 1,
    transition: override.transition ?? true,
    atMs: override.atMs ?? seq * 10,
    agentId: "root",
    parentTurnId: null,
    detail: override.detail ?? null,
    node: override.node,
    state: override.state,
  };
  return event as AgentNodeEventDto;
}

function fresh(): TurnObservation {
  return createObservation("conv-1", "conv-1:1", 0);
}

const STAGES = ["find", "validate", "gateway", "execute", "persist"] as const;

const APPROVAL: ToolApprovalRequestDto = {
  conversationId: "conv-1",
  turnId: "conv-1:1",
  callId: "call-1",
  toolName: "read_file",
  arguments: null,
  intents: [],
  timeoutMs: 0,
  context: {
    tier: "sandbox",
    behavior: { readOnly: true, destructive: false, openWorld: false },
    reasons: [],
    preview: null,
    extensions: [],
  },
};

const QUESTION: AgentQuestionRequestDto = {
  conversationId: "conv-1",
  turnId: "conv-1:1",
  callId: "call-1",
  questions: [],
  timeoutMs: 0,
};

function toolCall(
  phase: "awaitingApproval" | "awaitingAnswer" | "approvalClosed",
): AgentToolCallDto {
  const base = {
    conversationId: "conv-1",
    turnId: "conv-1:1",
    iteration: 1,
    callId: "call-1",
    name: "read_file",
  };
  if (phase === "awaitingApproval")
    return { ...base, payload: { phase, approval: APPROVAL } };
  if (phase === "awaitingAnswer")
    return { ...base, payload: { phase, question: QUESTION } };
  return { ...base, payload: { phase, timeoutMs: 0 } };
}

beforeEach(() => {
  seq = 0;
});

describe("reducer — 终态保护（G4 / O2）", () => {
  it("终态之后的事件被丢弃，且返回原引用（不写格子、不写时间线）", () => {
    let obs = fresh();
    obs = applyNodeEvent(
      obs,
      ev({ node: "ladder", state: "running", iteration: 1, atMs: 10 }),
    );
    obs = applyNodeEvent(
      obs,
      ev({ node: "ladder", state: "finished", iteration: 1, atMs: 20 }),
    );
    const settled = obs;
    const timelineLength = obs.timeline.length;

    const next = applyNodeEvent(
      obs,
      ev({ node: "ladder", state: "running", iteration: 1, atMs: 30 }),
    );
    expect(next).toBe(settled);
    expect(next.timeline.length).toBe(timelineLength);
    expect(next.cells.get("ladder:1")?.state).toBe("finished");
  });

  it("O2 例外：persist-assistant 重复 finished 只累加 rows / messageIds", () => {
    let obs = fresh();
    obs = applyNodeEvent(
      obs,
      ev({
        node: "persist-assistant",
        state: "running",
        iteration: 1,
        atMs: 10,
      }),
    );
    obs = applyNodeEvent(
      obs,
      ev({
        node: "persist-assistant",
        state: "finished",
        iteration: 1,
        atMs: 20,
        detail: { kind: "persist", rows: 1, messageIds: ["m1"] },
      }),
    );
    const first = obs.cells.get("persist-assistant:1");
    expect(first?.state).toBe("finished");
    expect(first?.sinceMs).toBe(10);

    obs = applyNodeEvent(
      obs,
      ev({
        node: "persist-assistant",
        state: "finished",
        iteration: 1,
        atMs: 30,
        detail: { kind: "persist", rows: 1, messageIds: ["m2"] },
      }),
    );
    const merged = obs.cells.get("persist-assistant:1");
    expect(merged?.state).toBe("finished");
    expect(merged?.sinceMs).toBe(10);
    expect(merged?.detail).toEqual({
      kind: "persist",
      rows: 2,
      messageIds: ["m1", "m2"],
    });

    const before = obs;
    const noop = applyNodeEvent(
      obs,
      ev({
        node: "persist-assistant",
        state: "finished",
        iteration: 1,
        atMs: 40,
        detail: { kind: "persist", rows: 0, messageIds: [] },
      }),
    );
    expect(noop).toBe(before);
  });

  it("其它节点重复 finished 一律被终态保护丢弃", () => {
    let obs = fresh();
    obs = applyNodeEvent(
      obs,
      ev({ node: "brake", state: "running", iteration: 1, atMs: 10 }),
    );
    obs = applyNodeEvent(
      obs,
      ev({
        node: "brake",
        state: "finished",
        iteration: 1,
        atMs: 20,
        detail: {
          kind: "brake",
          verdict: "continue",
          hook: null,
          budget: {
            remainingIterations: null,
            remainingToolCalls: null,
            remainingWallClockMs: null,
          },
        },
      }),
    );
    const settled = obs;
    const next = applyNodeEvent(
      obs,
      ev({ node: "brake", state: "finished", iteration: 1, atMs: 30 }),
    );
    expect(next).toBe(settled);
  });
});

describe("reducer — 未知取值（G8 / I55 §4.8 第 3 条）", () => {
  it("未知 node 进时间线、不建格子，cells 引用不变", () => {
    const obs = fresh();
    const next = applyNodeEvent(
      obs,
      unknownEv({ node: "orchestrator", state: "running" }),
    );

    expect(next).not.toBe(obs);
    expect(next.cells).toBe(obs.cells);
    expect(next.cells.size).toBe(0);
    expect(next.timeline).toHaveLength(1);
    expect(next.timeline[0]).toMatchObject({
      node: "orchestrator",
      state: "running",
      source: "observed",
    });
  });

  it("未知 state 进时间线、不建格子，也不触发 pending 推导", () => {
    const obs = fresh();
    const next = applyNodeEvent(
      obs,
      unknownEv({ node: "pipeline", state: "retrying" }),
    );

    expect(next.cells).toBe(obs.cells);
    expect(next.cells.size).toBe(0);
    expect(next.timeline).toHaveLength(1);
    expect(next.timeline[0].state).toBe("retrying");
  });
});

describe("reducer — transition:false 只进时间线（G12 读侧 / I55 §4.6 第 5 条）", () => {
  it("并发批次的 5N 条 StageEntered 不重置格子状态与 sinceMs", () => {
    let obs = fresh();
    obs = applyNodeEvent(
      obs,
      ev({ node: "pipeline", state: "running", iteration: 1, atMs: 100 }),
    );
    const runningCell = obs.cells.get("pipeline:1");
    expect(runningCell?.state).toBe("running");
    expect(runningCell?.sinceMs).toBe(100);

    const batches = [1, 2];
    for (const iteration of batches) {
      for (const [index, stage] of STAGES.entries()) {
        obs = applyNodeEvent(
          obs,
          ev({
            node: "pipeline",
            state: "running",
            iteration,
            transition: false,
            atMs: 200 + index,
            detail: {
              kind: "pipeline",
              stage,
              index: index + 1,
              total: STAGES.length,
              toolName: "read_file",
              toolCallId: `call-${index}`,
            },
          }),
        );
      }
    }

    expect(obs.cells.get("pipeline:1")).toBe(runningCell);
    expect(obs.timeline.filter((fact) => !fact.transition)).toHaveLength(
      5 * batches.length,
    );
    expect(obs.timeline.filter((fact) => fact.transition)).toHaveLength(1);
  });

  it("过程事实不会凭空建格子", () => {
    const obs = fresh();
    const next = applyNodeEvent(
      obs,
      ev({ node: "pipeline", state: "running", transition: false }),
    );
    expect(next.cells.size).toBe(0);
    expect(next.timeline).toHaveLength(1);
  });
});

describe("reducer — waiting 三来源（I55 §5.2）", () => {
  function withPipelineRunning(): TurnObservation {
    return applyNodeEvent(
      fresh(),
      ev({ node: "pipeline", state: "running", iteration: 1, atMs: 100 }),
    );
  }

  it("审批等待：pipeline running → waiting，审批关闭回 running 且 sinceMs 不变", () => {
    let obs = withPipelineRunning();
    obs = applyToolCall(obs, toolCall("awaitingApproval"));
    const waiting = obs.cells.get("pipeline:1");
    expect(waiting?.state).toBe("waiting");
    expect(waiting?.source).toBe("observed");
    expect(waiting?.sinceMs).toBe(100);

    obs = applyToolCall(obs, toolCall("approvalClosed"));
    const resumed = obs.cells.get("pipeline:1");
    expect(resumed?.state).toBe("running");
    expect(resumed?.sinceMs).toBe(100);
  });

  it("作答等待：pipeline running → waiting", () => {
    let obs = withPipelineRunning();
    obs = applyToolCall(obs, toolCall("awaitingAnswer"));
    expect(obs.cells.get("pipeline:1")?.state).toBe("waiting");
  });

  it("重复等待相位无变化时返回原引用", () => {
    let obs = withPipelineRunning();
    obs = applyToolCall(obs, toolCall("awaitingApproval"));
    const before = obs;
    expect(applyToolCall(obs, toolCall("awaitingApproval"))).toBe(before);
  });

  it("模型内层重试等待：model running ⇄ waiting 由节点事件表达，sinceMs 不重置", () => {
    let obs = fresh();
    obs = applyNodeEvent(
      obs,
      ev({ node: "model", state: "running", iteration: 1, atMs: 100 }),
    );
    obs = applyNodeEvent(
      obs,
      ev({
        node: "model",
        state: "waiting",
        iteration: 1,
        atMs: 300,
        detail: {
          kind: "model",
          attempt: 2,
          retryReason: "rate_limit",
          promptTokens: null,
          completionTokens: null,
          finishReason: null,
          toolCallCount: 0,
        },
      }),
    );
    expect(obs.cells.get("model:1")).toMatchObject({
      state: "waiting",
      sinceMs: 100,
    });

    obs = applyNodeEvent(
      obs,
      ev({ node: "model", state: "running", iteration: 1, atMs: 400 }),
    );
    expect(obs.cells.get("model:1")?.sinceMs).toBe(100);
  });
});

describe("reducer — source 证据等级（G13 / I55 §6.6）", () => {
  it("derived（pending 推导）→ observed 升级，且不回退", () => {
    let obs = applyNodeEvent(
      fresh(),
      ev({ node: "model", state: "running", iteration: 1, atMs: 100 }),
    );
    expect(obs.cells.get("budget-split:1")).toMatchObject({
      state: "pending",
      source: "derived",
      sinceMs: 0,
    });

    obs = applyNodeEvent(
      obs,
      ev({ node: "budget-split", state: "running", iteration: 1, atMs: 200 }),
    );
    expect(obs.cells.get("budget-split:1")).toMatchObject({
      state: "running",
      source: "observed",
      sinceMs: 200,
    });

    const before = obs;
    const downgraded = applyAssumedCell(obs, "budget-split", 1, "running", 999);
    expect(downgraded).toBe(before);
    expect(before.cells.get("budget-split:1")?.source).toBe("observed");
  });

  it("assumed（补拉点亮）→ observed 升级，并改用真实 atMs 作 sinceMs", () => {
    const restored = restoreObservation(null, {
      conversationId: "conv-1",
      turnId: "conv-1:1",
      turnIndex: 1,
      phase: "streaming",
      hasPendingToolCalls: false,
      atMs: 100,
    });
    expect(restored.cells.get("model:1")).toMatchObject({
      state: "running",
      source: "assumed",
      sinceMs: 100,
    });

    const observed = applyNodeEvent(
      restored,
      ev({ node: "model", state: "running", iteration: 1, atMs: 700 }),
    );
    expect(observed.cells.get("model:1")).toMatchObject({
      source: "observed",
      sinceMs: 700,
    });
  });

  it("derived 的 skipped 不被 assumed 补拉改写", () => {
    let obs = applyNodeEvent(
      fresh(),
      ev({ node: "branch", state: "running", iteration: 1, atMs: 100 }),
    );
    obs = applyNodeEvent(
      obs,
      ev({
        node: "branch",
        state: "finished",
        iteration: 1,
        atMs: 200,
        detail: {
          kind: "branch",
          hasToolCalls: false,
          wrapUp: false,
          toolCallCount: 0,
        },
      }),
    );
    const toolBranchNodes = [
      "budget-split",
      "pipeline",
      "after-tool",
      "persist-tools",
      "after-iteration",
    ] as const;
    for (const node of toolBranchNodes) {
      expect(obs.cells.get(`${node}:1`)).toMatchObject({
        state: "skipped",
        source: "derived",
      });
    }

    const restored = restoreObservation(obs, {
      conversationId: "conv-1",
      turnId: "conv-1:1",
      turnIndex: 1,
      phase: "toolCalling",
      hasPendingToolCalls: true,
      atMs: 300,
    });
    expect(restored.cells).toBe(obs.cells);
  });
});

describe("reducer — delta 与实例收尾", () => {
  it("applyDeltaSeq 只记 lastSeq 与出字迭代，不触碰格子", () => {
    let obs = applyNodeEvent(
      fresh(),
      ev({ node: "model", state: "running", iteration: 1, atMs: 100 }),
    );
    const cells = obs.cells;
    obs = applyDeltaSeq(obs, 7, 1);
    expect(obs.lastSeq).toBe(7);
    expect(obs.lastDeltaIteration).toBe(1);
    expect(obs.cells).toBe(cells);

    const before = obs;
    expect(applyDeltaSeq(obs, 7, 1)).toBe(before);
  });

  it("session-state-changed 只改实例 status，不臆造格子终态（O2 / O5）", () => {
    let obs = applyNodeEvent(
      fresh(),
      ev({ node: "model", state: "running", iteration: 1, atMs: 100 }),
    );
    const cells = obs.cells;

    obs = applySessionState(obs, "idle");
    expect(obs.status).toBe("completed");
    expect(obs.cells).toBe(cells);
    expect(obs.cells.get("model:1")?.state).toBe("running");

    const errored = { error: { message: "boom", recoverable: false } };
    obs = applySessionState(obs, errored);
    expect(obs.status).toBe("failed");

    const before = obs;
    expect(applySessionState(obs, errored)).toBe(before);
  });
});

describe("reducer — 时间线有界（I55 §6.7 第 5 条）", () => {
  function fillProcessFacts(count: number): TurnObservation {
    let obs = fresh();
    for (let index = 0; index < count; index += 1) {
      obs = applyNodeEvent(
        obs,
        ev({
          node: "pipeline",
          state: "running",
          transition: false,
          atMs: index,
        }),
      );
    }
    return obs;
  }

  it("超限后先丢最早的过程事实，并留下「已省略 N 条」合成事实", () => {
    const overflow = 50;
    const obs = fillProcessFacts(TIMELINE_LIMIT + overflow);

    const facts = obs.timeline.filter((fact) => !fact.synthetic);
    expect(facts).toHaveLength(TIMELINE_LIMIT);
    const marker = obs.timeline[0];
    expect(marker.synthetic).toBe(true);
    expect(marker.omitted).toBe(overflow);
    expect(marker.source).toBe("derived");
    expect(marker.seq).toBe(1);
  });

  it("状态迁移的终态永不丢弃", () => {
    let obs = fillProcessFacts(TIMELINE_LIMIT + 10);
    obs = applyNodeEvent(
      obs,
      ev({ node: "model", state: "finished", iteration: 1, atMs: 5_000 }),
    );

    expect(
      obs.timeline.some(
        (fact) => fact.node === "model" && fact.state === "finished",
      ),
    ).toBe(true);
    expect(obs.timeline.filter((fact) => !fact.synthetic)).toHaveLength(
      TIMELINE_LIMIT,
    );
    expect(obs.timeline[0].omitted).toBe(11);
  });
});
