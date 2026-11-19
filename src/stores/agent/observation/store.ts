import { create } from "zustand";
import type {
  AgentNodeEventDto,
  AgentToolCallDto,
  SessionStateDto,
} from "../../../types/protocol.contract";
import {
  applyDeltaSeq as reduceDeltaSeq,
  applyNodeEvent as reduceNodeEvent,
  applySessionState as reduceSessionState,
  applyToolCall as reduceToolCall,
  createObservation,
} from "./reducer";
import { restoreObservation, type RestoreInput } from "./restore";
import { observationKey, type TurnObservation } from "./types";

/** 观测存储最多保留的实例数（I55 §6.3 第 3 条）。 */
export const OBSERVATION_LIMIT = 32;

export interface ObservationState {
  /** `Map<observationKey, TurnObservation>`，key = `${conversationId}:${turnId}`。 */
  observations: Map<string, TurnObservation>;
  applyNodeEvent: (event: AgentNodeEventDto) => void;
  applyDeltaSeq: (
    conversationId: string,
    turnId: string,
    seq: number,
    iteration: number,
  ) => void;
  applyToolCall: (call: AgentToolCallDto) => void;
  applySessionState: (
    conversationId: string,
    turnId: string,
    state: SessionStateDto,
  ) => void;
  restore: (input: RestoreInput) => void;
  getObservation: (
    conversationId: string,
    turnId: string,
  ) => TurnObservation | null;
}

/**
 * 观测存储：`agent-node-event` 的**唯一**折叠点（I55 §6.7 第 1 条）。
 *
 * 三条纪律：
 * - 折叠**同步逐事件**完成，不排队、不合并、不丢弃（完整性）；
 * - 无真实变化时**不写回**（O11：下游靠引用相等跳过重算）；
 * - 存储是只读副本，任何 `send` / `cancel` 路径都不得读它（O6 / §6.5）。
 */
export const useObservationStore = create<ObservationState>()((set, get) => {
  /** 侧表：`observationKey → 最近活动序号`，只服务 LRU，不参与渲染。 */
  const recency = new Map<string, number>();
  let recencySeq = 0;

  function touch(key: string): void {
    recencySeq += 1;
    recency.set(key, recencySeq);
  }

  function oldestKey(keys: string[]): string | null {
    let oldest: string | null = null;
    let oldestAt = Infinity;
    for (const key of keys) {
      const at = recency.get(key) ?? 0;
      if (at < oldestAt) {
        oldestAt = at;
        oldest = key;
      }
    }
    return oldest;
  }

  /** 淘汰时只丢 `cells` / `timeline`，保留 `status` 摘要（I55 §6.3 第 3 条）。 */
  function summarize(obs: TurnObservation): TurnObservation {
    if (obs.evicted) return obs;
    return { ...obs, cells: new Map(), timeline: [], evicted: true };
  }

  function ensureCapacity(
    observations: Map<string, TurnObservation>,
    incomingKey: string,
  ): Map<string, TurnObservation> {
    if (observations.size < OBSERVATION_LIMIT) return observations;
    const next = new Map(observations);

    const fullKeys = [...next.entries()]
      .filter(([, obs]) => !obs.evicted)
      .map(([key]) => key);
    const lruFull = oldestKey(fullKeys);
    if (lruFull !== null && lruFull !== incomingKey) {
      next.set(lruFull, summarize(next.get(lruFull)!));
    }

    while (next.size >= OBSERVATION_LIMIT) {
      const victim = oldestKey([...next.keys()]);
      if (victim === null || victim === incomingKey) break;
      next.delete(victim);
      recency.delete(victim);
    }
    return next;
  }

  function pruneRecency(observations: Map<string, TurnObservation>): void {
    for (const key of recency.keys()) {
      if (!observations.has(key)) recency.delete(key);
    }
  }

  /**
   * 统一写回：无变化不写（O11），新键先腾容量。
   * `evicted` 摘要收到新事件即重新成为完整实例。
   */
  function commit(
    key: string,
    prev: TurnObservation | undefined,
    next: TurnObservation,
  ): void {
    if (prev && next === prev) return;
    const observations = get().observations;
    const map = prev ? observations : ensureCapacity(observations, key);
    const updated = new Map(map);
    updated.set(key, prev?.evicted ? { ...next, evicted: false } : next);
    touch(key);
    pruneRecency(updated);
    set({ observations: updated });
  }

  return {
    observations: new Map(),

    applyNodeEvent: (event) => {
      const key = observationKey(event.conversationId, event.turnId);
      const prev = get().observations.get(key);
      const base =
        prev ??
        createObservation(event.conversationId, event.turnId, event.atMs);
      commit(key, prev, reduceNodeEvent(base, event));
    },

    applyDeltaSeq: (conversationId, turnId, seq, iteration) => {
      const key = observationKey(conversationId, turnId);
      const prev = get().observations.get(key);
      if (!prev) return;
      commit(key, prev, reduceDeltaSeq(prev, seq, iteration));
    },

    applyToolCall: (call) => {
      const key = observationKey(call.conversationId, call.turnId);
      const prev = get().observations.get(key);
      if (!prev) return;
      commit(key, prev, reduceToolCall(prev, call));
    },

    applySessionState: (conversationId, turnId, state) => {
      const key = observationKey(conversationId, turnId);
      const prev = get().observations.get(key);
      if (!prev) return;
      commit(key, prev, reduceSessionState(prev, state));
    },

    restore: (input) => {
      const key = observationKey(input.conversationId, input.turnId);
      const prev = get().observations.get(key);
      commit(key, prev, restoreObservation(prev ?? null, input));
    },

    getObservation: (conversationId, turnId) =>
      get().observations.get(observationKey(conversationId, turnId)) ?? null,
  };
});
