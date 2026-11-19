import type { NodeFact, TurnObservation } from "../types";

/**
 * 时间线投影（纯函数，I55 §6.2）。
 *
 * 按 `seq` 排序（与 `chat-delta` 共用同一单调序列，I55 §4.2），
 * `transition` 原样保留：inspector 据此区分「状态迁移」与「过程事实」（§4.6 第 5 条）。
 * 有界丢弃留下的「已省略 N 条」合成事实恒在最前（§6.7 第 5 条）。
 */
export function projectTimeline(obs: TurnObservation): NodeFact[] {
  const marker = obs.timeline.find((fact) => fact.synthetic);
  const facts = obs.timeline
    .filter((fact) => !fact.synthetic)
    .sort((a, b) => a.seq - b.seq);
  return marker ? [marker, ...facts] : facts;
}
