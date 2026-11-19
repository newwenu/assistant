import { useEffect, useState } from "react";

type Listener = () => void;

const listeners = new Set<Listener>();
let timer: number | null = null;
let timerInterval = 0;
let current = Date.now();

function notify(): void {
  current = Date.now();
  for (const listener of listeners) listener();
}

function ensureTimer(intervalMs: number): void {
  if (timer != null && timerInterval === intervalMs) return;
  if (timer != null) window.clearInterval(timer);
  current = Date.now();
  timerInterval = intervalMs;
  timer = window.setInterval(notify, intervalMs);
}

function releaseTimer(): void {
  if (timer == null || listeners.size > 0) return;
  window.clearInterval(timer);
  timer = null;
  timerInterval = 0;
}

/**
 * 订阅共享 ticker（I60 §3.6）：全应用最多一个 `setInterval`，
 * 首个订阅者启动、最后一个退订时清理。返回退订函数。
 */
export function subscribeNowTick(
  listener: Listener,
  intervalMs = 1000,
): () => void {
  listeners.add(listener);
  ensureTimer(intervalMs);
  return () => {
    listeners.delete(listener);
    releaseTimer();
  };
}

/**
 * `enabled` 为真时每 `intervalMs` 返回一次新的当前时刻；否则返回常量且不订阅。
 */
export function useNowTick(enabled: boolean, intervalMs = 1000): number {
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (!enabled) return;
    const unsubscribe = subscribeNowTick(() => setNow(current), intervalMs);
    setNow(Date.now());
    return unsubscribe;
  }, [enabled, intervalMs]);

  return now;
}
