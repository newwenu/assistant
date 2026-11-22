import { describe, expect, it } from "vitest";
import {
  aggregatePhasesOf,
  closedCountOf,
  phaseToneOf,
} from "@/features/canvas/presets/phaseAggregate";
import {
  STAGE_PHASES,
  STAGE_PHASE_COUNT,
  STAGE_PHASE_NODE_COUNT,
} from "@/features/canvas/presets/stagePhases";
import { AGENT_NODE_TEMPLATE } from "@/stores/agent/observation/template";
import type { NodeState } from "@/stores/agent/observation/types";

describe("STAGE_PHASES", () => {
  it("declares exactly five display phases in a fixed order", () => {
    expect(STAGE_PHASE_COUNT).toBe(5);
    expect(STAGE_PHASES.map((phase) => phase.id)).toEqual([
      "settle",
      "decide",
      "commit",
      "tools",
      "wrap",
    ]);
  });

  it("covers the twelve L1 nodes bijectively", () => {
    const nodeIds = STAGE_PHASES.flatMap((phase) => phase.nodes);

    expect(STAGE_PHASE_NODE_COUNT).toBe(AGENT_NODE_TEMPLATE.length);
    expect(nodeIds.length).toBe(AGENT_NODE_TEMPLATE.length);
    expect(new Set(nodeIds).size).toBe(AGENT_NODE_TEMPLATE.length);

    const templateIds = AGENT_NODE_TEMPLATE.map((entry) => entry.id);
    expect([...nodeIds].sort()).toEqual([...templateIds].sort());
  });

  it("keeps phases in template order, so the track reads top-down as progress", () => {
    const orders = AGENT_NODE_TEMPLATE.map((entry) => entry.order);
    let last = -1;
    for (const phase of STAGE_PHASES) {
      for (const node of phase.nodes) {
        const order = AGENT_NODE_TEMPLATE.find(
          (entry) => entry.id === node,
        )?.order;
        expect(order).toBeGreaterThan(last);
        last = order as number;
      }
    }
    expect(orders.length).toBe(12);
  });
});

describe("phaseToneOf", () => {
  it("lets error outrank every other state", () => {
    expect(phaseToneOf(["finished", "error", "running"])).toBe("danger");
  });

  it("lets running outrank waiting and pending", () => {
    expect(phaseToneOf(["finished", "running", "pending"])).toBe("active");
    expect(phaseToneOf(["waiting", "pending"])).toBe("warning");
  });

  it("marks an all-skipped phase as muted", () => {
    expect(phaseToneOf(["skipped", "cancelled"])).toBe("muted");
    expect(phaseToneOf(["skipped", "finished"])).toBe("success");
  });

  it("treats all terminal states as success", () => {
    expect(phaseToneOf(["finished", "finished"])).toBe("success");
    expect(phaseToneOf(["finished", "error"])).toBe("danger");
  });

  it("keeps any non-terminal state at default", () => {
    expect(phaseToneOf(["pending"])).toBe("default");
    expect(phaseToneOf([])).toBe("default");
  });
});

describe("closedCountOf", () => {
  it("counts only terminal states", () => {
    expect(closedCountOf(["finished", "running"])).toBe(1);
    expect(closedCountOf(["skipped", "cancelled", "error"])).toBe(3);
    expect(closedCountOf(["pending", "waiting"])).toBe(0);
  });
});

describe("aggregatePhasesOf", () => {
  it("aggregates each phase from the template without a projection", () => {
    const phases = aggregatePhasesOf(() => "pending");

    expect(phases.map((phase) => phase.id)).toEqual([
      "settle",
      "decide",
      "commit",
      "tools",
      "wrap",
    ]);
    expect(phases.map((phase) => phase.total)).toEqual([2, 3, 2, 4, 1]);
    expect(phases.map((phase) => phase.closed)).toEqual([0, 0, 0, 0, 0]);
    for (const phase of phases) {
      expect(phase.tone).toBe("default");
    }
  });

  it("closes the count once its group reaches terminal states", () => {
    const stateOf = (node: string): NodeState =>
      node === "admit" || node === "prepare" ? "finished" : "pending";
    const [settle, decide] = aggregatePhasesOf(stateOf);

    expect(settle.tone).toBe("success");
    expect(settle.closed).toBe(2);
    expect(decide.tone).toBe("default");
    expect(decide.closed).toBe(0);
  });

  it("reports the failing phase as danger even when others are finished", () => {
    const stateOf = (node: string): NodeState =>
      node === "model" ? "error" : "finished";
    const phases = aggregatePhasesOf(stateOf);
    const decide = phases.find((phase) => phase.id === "decide");

    expect(decide?.tone).toBe("danger");
    expect(decide?.closed).toBe(3);
  });
});
