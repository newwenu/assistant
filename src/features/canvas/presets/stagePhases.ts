import type { AgentNodeId } from "@/stores/agent/observation/types";
import { AGENT_NODE_TEMPLATE } from "@/stores/agent/observation/template";
import type { I18nPath } from "@/i18n";

/**
 * 显示阶段：12 个 L1 节点归并为 5 个显示阶段。
 *
 * 归并依据是「同一决策闭环 + 同量级耗时」——`brake` / `persist-assistant` /
 * `persist-tools` 耗时在毫秒级，与秒级的 `model` 各占一格会让视觉权重失真。
 * 与 `AGENT_NODE_TEMPLATE` 同层级：编译期常量，不进 wire、无运行时注册表（O4）。
 */
export interface StagePhaseSpec {
  id: StagePhaseId;
  nodes: readonly AgentNodeId[];
  labelKey: I18nPath;
}

export type StagePhaseId = "settle" | "decide" | "commit" | "tools" | "wrap";

export const STAGE_PHASES = [
  {
    id: "settle",
    nodes: ["admit", "prepare"],
    labelKey: "canvas.phase.settle",
  },
  {
    id: "decide",
    nodes: ["brake", "ladder", "model"],
    labelKey: "canvas.phase.decide",
  },
  {
    id: "commit",
    nodes: ["persist-assistant", "branch"],
    labelKey: "canvas.phase.commit",
  },
  {
    id: "tools",
    nodes: ["budget-split", "pipeline", "after-tool", "persist-tools"],
    labelKey: "canvas.phase.tools",
  },
  { id: "wrap", nodes: ["after-iteration"], labelKey: "canvas.phase.wrap" },
] as const satisfies readonly StagePhaseSpec[];

/** 归并表的节点总数（覆盖度锁的一半：与模板长度必须相等，A2）。 */
export const STAGE_PHASE_NODE_COUNT = STAGE_PHASES.reduce(
  (sum, phase) => sum + phase.nodes.length,
  0,
);

/** 显示阶段数（阶段轨道行数，A1）。 */
export const STAGE_PHASE_COUNT = STAGE_PHASES.length;

/** 12 个 L1 的模板顺序（0 起）：用于阶段名旁的序号与 `title` 里的明细。 */
const ORDER_BY_ID = new Map<AgentNodeId, number>(
  AGENT_NODE_TEMPLATE.map((entry) => [entry.id, entry.order]),
);

/** 模板序号 → 圈号字符（①…⑫）。 */
const ORDER_BADGES = [
  "①",
  "②",
  "③",
  "④",
  "⑤",
  "⑥",
  "⑦",
  "⑧",
  "⑨",
  "⑩",
  "⑪",
  "⑫",
] as const;

/** L1 节点在模板里的序号字符（如 `admit` → `①`）。 */
export function orderBadgeOf(node: AgentNodeId): string {
  const order = ORDER_BY_ID.get(node);
  return order === undefined ? "" : (ORDER_BADGES[order] ?? "");
}

/** 显示阶段在轨道上的序号（1 起，A1 顺序固定）。 */
export function phaseOrdinalOf(id: StagePhaseId): number {
  return STAGE_PHASES.findIndex((phase) => phase.id === id) + 1;
}
