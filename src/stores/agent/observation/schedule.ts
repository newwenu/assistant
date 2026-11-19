import { useObservationStore } from "./store";
import { observationKey, type NodeFact, type TurnObservation } from "./types";
import { projectGraph, type GraphView } from "./projections/graph";
import { projectTimeline } from "./projections/timeline";
import {
  projectTurnSummary,
  type TurnSummaryView,
} from "./projections/summary";

/** 三投影的一次性产物（同一份 `TurnObservation` 派生，I55 §6.2）。 */
export interface ObservationProjections {
  graph: GraphView;
  timeline: NodeFact[];
  summary: TurnSummaryView;
}

export type ProjectionsListener = (
  projections: ObservationProjections | null,
) => void;

interface Subscriber {
  listener: ProjectionsListener;
  observation: TurnObservation | null;
  projections: ObservationProjections | null;
}

/** 三投影一次算齐：互不引用，输入只有同一份 `TurnObservation`（I55 §6.2）。 */
export function projectObservation(
  obs: TurnObservation,
): ObservationProjections {
  return {
    graph: projectGraph(obs),
    timeline: projectTimeline(obs),
    summary: projectTurnSummary(obs),
  };
}

/**
 * 读当前投影（**不缓存**，I55 §6.7 末段）：需要缓存的调用方自己按 `Object.is` 判失效。
 * 无实例时返回 `null`（空态）。
 */
export function readProjections(
  conversationId: string,
  turnId: string,
): ObservationProjections | null {
  const obs =
    useObservationStore
      .getState()
      .observations.get(observationKey(conversationId, turnId)) ?? null;
  return obs ? projectObservation(obs) : null;
}

const subscribersByKey = new Map<string, Set<Subscriber>>();
const pendingKeys = new Set<string>();
let storeUnsubscribe: (() => void) | null = null;
let flushQueued = false;

/**
 * 订阅投影更新（I55 §6.7 第 1 / 3 / 4 条）——本层是**唯一**的投影调度点：
 *
 * - 折叠（`applyNodeEvent` / `applyDeltaSeq` / `applyToolCall`）由 store 逐事件同步完成，
 *   本层只读结果，**不丢事件**（时间线事实数恒等于写入事件数）；
 * - 三投影按帧合并：同一帧内多次变更只重算一次（`requestAnimationFrame`，无 rAF 环境退化为宏任务）；
 * - 引用相等抑制：`TurnObservation` 引用未变（O11）即跳过重算，下游拿到的是同一个对象；
 * - `GraphCanvas` / `InspectorPanel` **禁止**各自 `listen(agent-node-event)`，只订阅本函数。
 */
export function subscribeProjections(
  conversationId: string,
  turnId: string,
  listener: ProjectionsListener,
): () => void {
  const key = observationKey(conversationId, turnId);
  const observation =
    useObservationStore.getState().observations.get(key) ?? null;
  const subscriber: Subscriber = {
    listener,
    observation,
    projections: observation ? projectObservation(observation) : null,
  };

  let set = subscribersByKey.get(key);
  if (!set) {
    set = new Set();
    subscribersByKey.set(key, set);
  }
  set.add(subscriber);
  ensureStoreSubscription();

  listener(subscriber.projections);

  return () => {
    const current = subscribersByKey.get(key);
    if (!current) return;
    current.delete(subscriber);
    if (current.size === 0) {
      subscribersByKey.delete(key);
      pendingKeys.delete(key);
      releaseStoreSubscription();
    }
  };
}

function ensureStoreSubscription(): void {
  if (storeUnsubscribe) return;
  storeUnsubscribe = useObservationStore.subscribe((state, prev) => {
    if (state.observations === prev.observations) return;
    for (const key of subscribersByKey.keys()) {
      const before = prev.observations.get(key);
      const after = state.observations.get(key);
      if (before === after) continue;
      pendingKeys.add(key);
    }
    if (pendingKeys.size > 0) scheduleFlush();
  });
}

function releaseStoreSubscription(): void {
  if (subscribersByKey.size > 0) return;
  storeUnsubscribe?.();
  storeUnsubscribe = null;
  pendingKeys.clear();
}

function scheduleFlush(): void {
  if (flushQueued) return;
  flushQueued = true;
  const run = (): void => {
    flushQueued = false;
    flushPending();
  };
  if (typeof requestAnimationFrame === "function") {
    requestAnimationFrame(run);
  } else {
    setTimeout(run, 0);
  }
}

function flushPending(): void {
  const keys = [...pendingKeys];
  pendingKeys.clear();
  for (const key of keys) {
    const subscribers = subscribersByKey.get(key);
    if (!subscribers || subscribers.size === 0) continue;
    const observation =
      useObservationStore.getState().observations.get(key) ?? null;
    const stale = [...subscribers].filter(
      (subscriber) => subscriber.observation !== observation,
    );
    if (stale.length === 0) continue;
    const projections = observation ? projectObservation(observation) : null;
    for (const subscriber of stale) {
      subscriber.observation = observation;
      subscriber.projections = projections;
      subscriber.listener(projections);
    }
  }
}
