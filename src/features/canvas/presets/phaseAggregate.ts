import type { AgentNodeId, NodeState } from "@/stores/agent/observation/types";
import type { CardTone } from "@/features/canvas/types";
import {
  STAGE_PHASES,
  orderBadgeOf,
  type StagePhaseId,
  type StagePhaseSpec,
} from "./stagePhases";

/** 终态（写入后不可再覆盖）：只有终态计入「已闭合」。 */
const CLOSED_STATES: readonly NodeState[] = [
  "finished",
  "error",
  "skipped",
  "cancelled",
];

/**
 * L1 状态 → 显示阶段聚合状态：按优先级从高到低取第一条成立者。
 * 这里是设施与观测域的唯一接缝，产出只有 `CardTone`。
 */
export function phaseToneOf(states: readonly NodeState[]): CardTone {
  if (states.some((state) => state === "error")) return "danger";
  if (states.some((state) => state === "running")) return "active";
  if (states.some((state) => state === "waiting")) return "warning";
  if (
    states.length > 0 &&
    states.every((state) => state === "skipped" || state === "cancelled")
  ) {
    return "muted";
  }
  if (states.length > 0 && states.every((state) => isClosed(state))) {
    return "success";
  }
  return "default";
}

/** 已闭合 L1 数（`finished` / `error` / `skipped` / `cancelled`）。 */
export function closedCountOf(states: readonly NodeState[]): number {
  return states.filter((state) => isClosed(state)).length;
}

/** 显示阶段的原始聚合量：只有计数与色调，不含任何文案。 */
export interface PhaseAggregate {
  id: StagePhaseId;
  spec: StagePhaseSpec;
  tone: CardTone;
  closed: number;
  total: number;
  /** 组内 L1 的 `状态` 序列，供内容层拼 `title` 明细。 */
  nodeStates: readonly { node: AgentNodeId; state: NodeState }[];
}

/** 按归并表算出 5 个显示阶段（无投影时全部 `pending` → `default`）。 */
export function aggregatePhasesOf(
  stateOf: (node: AgentNodeId) => NodeState,
): PhaseAggregate[] {
  return STAGE_PHASES.map((spec) => {
    const nodeStates = spec.nodes.map((node) => ({
      node,
      state: stateOf(node),
    }));
    const states = nodeStates.map((entry) => entry.state);
    return {
      id: spec.id,
      spec,
      tone: phaseToneOf(states),
      closed: closedCountOf(states),
      total: spec.nodes.length,
      nodeStates,
    };
  });
}

/** 组内 L1 明细（进 `title`，不单独排版）：`①准入·进行中` 逐条以逗号分隔。 */
export function phaseDetailOf(
  aggregate: PhaseAggregate,
  nodeText: (node: AgentNodeId) => string,
  stateText: (state: NodeState) => string,
): string {
  return aggregate.nodeStates
    .map(
      ({ node, state }) =>
        `${orderBadgeOf(node)}${nodeText(node)}·${stateText(state)}`,
    )
    .join("，");
}

function isClosed(state: NodeState): boolean {
  return CLOSED_STATES.includes(state);
}
