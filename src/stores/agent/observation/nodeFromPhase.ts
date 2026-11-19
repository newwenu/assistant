import type { TurnPhaseDto } from "../../../types/protocol.contract";
import type { AgentNodeId, NodeState } from "./types";

export interface PhaseNode {
  node: AgentNodeId;
  state: NodeState;
}

/**
 * `TurnPhase` → 节点的**粗粒度**恢复映射（I55 §6.4）。
 *
 * **仅恢复用**：窗口重载 / 切会话后内存观测表为空，只能靠 `session-state-changed`
 * 的 `phase` 点灯；正常渲染一律以节点事件为准。
 * 这张映射**只允许出现在这一个函数里**，其它任何地方都不得按 phase 推节点。
 */
const PHASE_NODE = {
  starting: { node: "admit", state: "running" },
  thinking: { node: "ladder", state: "running" },
  streaming: { node: "model", state: "running" },
  toolCalling: { node: "pipeline", state: "running" },
  awaitingApproval: { node: "pipeline", state: "waiting" },
  compacting: { node: "ladder", state: "running" },
  finalizing: { node: "persist-assistant", state: "running" },
} satisfies Record<TurnPhaseDto, PhaseNode>;

export function nodeFromPhase(phase: TurnPhaseDto): PhaseNode {
  return PHASE_NODE[phase];
}
