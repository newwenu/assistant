import { STREAM_COALESCE_TIMER_MS } from "../../config/constants";

export interface CoalescibleDelta {
  conversation_id: string;
  attempt_id: string;
  message_id: string;
  seq: number;
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

export const textChunkScheduler: FlushScheduler = (flush) => {
  let done = false;
  const run = () => {
    if (done) return;
    done = true;
    flush();
  };
  if (typeof document !== "undefined" && document.hidden) {
    setTimeout(run, 0);
    return () => {
      done = true;
    };
  }
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

export interface CoalesceOptions<E extends CoalescibleDelta> {
  schedule?: FlushScheduler;
  accept?: (event: E) => boolean;
}

export interface ChatDeltaCoalescer<E extends CoalescibleDelta> {
  push: (event: E) => void;
  flush: () => void;
  dispose: () => void;
}

export function createChatDeltaCoalescer<E extends CoalescibleDelta>(
  deliver: (event: E) => void,
  schedule: FlushScheduler = frameScheduler,
  accept?: (event: E) => boolean,
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
    if (accept && !accept(event)) return;
    const last = queue[queue.length - 1];
    const eventKey = event.attempt_id;
    const lastKey = last ? last.event.attempt_id : undefined;
    if (
      last &&
      last.event.conversation_id === event.conversation_id &&
      lastKey === eventKey
    ) {
      last.event = { ...event, delta: `${last.event.delta}${event.delta}` };
    } else {
      queue.push({ event });
    }
    cancel ??= schedule(flush);
  };

  return { push, flush, dispose: flush };
}

const MAX_SEQ_ENTRIES = 64;

export function createSeqDedupFilter(): {
  accept: (event: CoalescibleDelta) => boolean;
  reset: () => void;
} {
  const lastSeqByRequest = new Map<string, number>();

  const accept = (event: CoalescibleDelta): boolean => {
    const streamKey = event.attempt_id;
    const key = `${event.conversation_id}:${streamKey}`;
    const last = lastSeqByRequest.get(key);
    if (last !== undefined && event.seq <= last) {
      return false;
    }
    lastSeqByRequest.delete(key);
    lastSeqByRequest.set(key, event.seq);
    while (lastSeqByRequest.size > MAX_SEQ_ENTRIES) {
      const oldest = lastSeqByRequest.keys().next().value;
      if (oldest === undefined) break;
      lastSeqByRequest.delete(oldest);
    }
    return true;
  };

  const reset = () => {
    lastSeqByRequest.clear();
  };

  return { accept, reset };
}
