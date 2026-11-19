import { useEffect, useMemo, useState } from "react";
import {
  readProjections,
  subscribeProjections,
  type ObservationProjections,
} from "@/stores/agent/observation/schedule";
import { useObservationStore } from "@/stores/agent/observation/store";
import type {
  TurnObservation,
  TurnObservationStatus,
} from "@/stores/agent/observation/types";

/** 列表项只取渲染需要的摘要字段，不把整份 `TurnObservation` 交给列表。 */
export interface ObservationInstanceView {
  conversationId: string;
  turnId: string;
  turnIndex: number | null;
  status: TurnObservationStatus;
  startedAtMs: number;
  evicted: boolean;
}

function toInstanceView(obs: TurnObservation): ObservationInstanceView {
  return {
    conversationId: obs.conversationId,
    turnId: obs.turnId,
    turnIndex: obs.turnIndex,
    status: obs.status,
    startedAtMs: obs.startedAtMs,
    evicted: obs.evicted ?? false,
  };
}

/**
 * 实例列表：读侧只读副本（I55 §6.3），按起点倒序。
 *
 * 这里**不**折叠、不写、不驱动任何控制路径（O6 / G7）。
 *
 * 选择器只取 `observations` 这个 Map 的引用（store 每次写回都换新 Map，
 * 未变化时不换），列表在 `useMemo` 里派生。**不能**把「构造新数组」放进选择器：
 * 每次求值都会得到新对象，浅比较永远不等，就会退化成无限重渲染
 * （`Maximum update depth exceeded`）。
 */
export function useObservationInstances(): ObservationInstanceView[] {
  const observations = useObservationStore((state) => state.observations);

  return useMemo(() => {
    const list: ObservationInstanceView[] = [];
    for (const obs of observations.values()) {
      list.push(toInstanceView(obs));
    }
    list.sort(
      (a, b) =>
        b.startedAtMs - a.startedAtMs || a.turnId.localeCompare(b.turnId),
    );
    return list;
  }, [observations]);
}

/**
 * 订阅三投影（I55 §6.7 第 1 / 3 / 4 条）。
 *
 * 组件**不**自己 `listen(agent-node-event)`，只订阅调度层；无实例时返回 `null`（空态）。
 */
export function useProjections(
  conversationId: string | null,
  turnId: string | null,
): ObservationProjections | null {
  const [projections, setProjections] = useState<ObservationProjections | null>(
    () =>
      conversationId && turnId ? readProjections(conversationId, turnId) : null,
  );

  useEffect(() => {
    if (!conversationId || !turnId) {
      setProjections(null);
      return;
    }
    setProjections(readProjections(conversationId, turnId));
    return subscribeProjections(conversationId, turnId, setProjections);
  }, [conversationId, turnId]);

  return projections;
}
