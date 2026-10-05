import { STREAM_COALESCE_TIMER_MS } from "../config/constants";

export interface CoalescibleDelta {
  conversation_id: string;
  message_id: string;
  delta: string;
}

type Pending<E extends CoalescibleDelta> = { event: E };

export type FlushScheduler = (flush: () => void) => () => void;

export const frameScheduler: FlushScheduler = (flush) => {
  let done = false;
  const run = () => {
    if (done) return;
    done = true;
    flush();
  };
  const raf =
    typeof window !== "undefined" &&
    typeof window.requestAnimationFrame === "function"
      ? window.requestAnimationFrame(run)
      : undefined;
  const timer = setTimeout(run, STREAM_COALESCE_TIMER_MS);
  return () => {
    done = true;
    if (raf !== undefined && typeof window !== "undefined") {
      window.cancelAnimationFrame(raf);
    }
    clearTimeout(timer);
  };
};

export interface ChatDeltaCoalescer<E extends CoalescibleDelta> {
  push: (event: E) => void;
  flush: () => void;
  dispose: () => void;
}

export function createChatDeltaCoalescer<E extends CoalescibleDelta>(
  deliver: (event: E) => void,
  schedule: FlushScheduler = frameScheduler,
): ChatDeltaCoalescer<E> {
  let queue: Pending<E>[] = [];
  let cancel: (() => void) | null = null;

  const flush = () => {
    cancel?.();
    cancel = null;
    if (queue.length === 0) return;
    const drained = queue;
    queue = [];
    for (const item of drained) deliver(item.event);
  };

  const push = (event: E) => {
    const last = queue[queue.length - 1];
    if (
      last &&
      last.event.conversation_id === event.conversation_id &&
      last.event.message_id === event.message_id
    ) {
      last.event = { ...event, delta: `${last.event.delta}${event.delta}` };
    } else {
      queue.push({ event });
    }
    cancel ??= schedule(flush);
  };

  return { push, flush, dispose: flush };
}
