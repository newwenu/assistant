import type { TurnPhaseDto } from "../../../types/protocol.contract";
import {
  applyAssumedCell,
  applyTurnIndex,
  createObservation,
  currentIterationOf,
} from "./reducer";
import { nodeFromPhase } from "./nodeFromPhase";
import { isIterationNode } from "./template";
import type { TurnObservation } from "./types";

/** 补拉输入：全部来自既有快照与事件，**没有**新的查询命令（I55 §6.4 第 3 条）。 */
export interface RestoreInput {
  conversationId: string;
  turnId: string;
  turnIndex: number | null;
  /** `session-state-changed` 的 `Processing { phase }`；非处理中为 `null`。 */
  phase: TurnPhaseDto | null;
  /** `onPendingToolCalls` 的结论：存在挂起工具调用即把 `pipeline` 置 `waiting`（N12）。 */
  hasPendingToolCalls: boolean;
  /** 补拉时刻（相对 turn 起点毫秒）：起点未知，只作 `sinceMs` 占位，不参与耗时分解。 */
  atMs: number;
}

/**
 * 窗口重载 / 切会话后的补拉（I55 §6.4）。
 *
 * 补拉产生的所有格子 `source = assumed`：**禁止**把它们当成 `observed` 渲染，
 * 也**禁止**用它们算耗时分解（起点是猜的，I55 §6.6 第 4 条）。
 * 收到真实事件后由 reducer 升为 `observed`（只升不降）。
 */
export function restoreObservation(
  obs: TurnObservation | null,
  input: RestoreInput,
): TurnObservation {
  let next =
    obs ?? createObservation(input.conversationId, input.turnId, input.atMs);
  next = applyTurnIndex(next, input.turnIndex);

  if (input.phase) {
    const { node, state } = nodeFromPhase(input.phase);
    next = applyAssumedCell(
      next,
      node,
      isIterationNode(node) ? currentIterationOf(next) : 0,
      state,
      input.atMs,
    );
  }

  if (input.hasPendingToolCalls) {
    next = applyAssumedCell(
      next,
      "pipeline",
      currentIterationOf(next),
      "waiting",
      input.atMs,
    );
  }

  return next;
}
