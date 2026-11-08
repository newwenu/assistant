import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  createChatDeltaCoalescer,
  createSeqDedupFilter,
  frameScheduler,
  type CoalescibleDelta,
} from "../core/streamCoalescer";

interface TestDelta extends CoalescibleDelta {
  delta: string;
}

function mkDelta(
  overrides: Partial<TestDelta> & Pick<TestDelta, "delta">,
): TestDelta {
  return {
    conversationId: "c1",
    attemptId: "a1",
    messageId: "m1",
    iteration: 1,
    seq: 0,
    ...overrides,
  };
}

function immediateScheduler(flush: () => void): () => void {
  flush();
  return () => {};
}

function manualScheduler(): {
  schedule: (flush: () => void) => () => void;
  trigger: () => void;
} {
  let pending: (() => void) | null = null;
  return {
    schedule(flush: () => void) {
      pending = flush;
      return () => {
        pending = null;
      };
    },
    trigger() {
      if (pending) {
        const fn = pending;
        pending = null;
        fn();
      }
    },
  };
}

describe("createChatDeltaCoalescer", () => {
  it("delivers a single event on flush", () => {
    const deliver = vi.fn();
    const coalescer = createChatDeltaCoalescer<TestDelta>(
      deliver,
      immediateScheduler,
    );

    coalescer.push(mkDelta({ delta: "hello" }));

    expect(deliver).toHaveBeenCalledTimes(1);
    expect(deliver).toHaveBeenCalledWith(mkDelta({ delta: "hello" }));
  });

  it("coalesces consecutive deltas with same conversation and message id", () => {
    const deliver = vi.fn();
    const { schedule, trigger } = manualScheduler();
    const coalescer = createChatDeltaCoalescer<TestDelta>(deliver, schedule);

    coalescer.push(mkDelta({ delta: "hel" }));
    coalescer.push(mkDelta({ delta: "lo" }));
    coalescer.push(mkDelta({ delta: " world" }));

    trigger();

    expect(deliver).toHaveBeenCalledTimes(1);
    expect(deliver).toHaveBeenCalledWith(mkDelta({ delta: "hello world" }));
  });

  it("does not coalesce deltas with different attempt ids", () => {
    const deliver = vi.fn();
    const { schedule, trigger } = manualScheduler();
    const coalescer = createChatDeltaCoalescer<TestDelta>(deliver, schedule);

    coalescer.push(mkDelta({ delta: "hello" }));
    coalescer.push(mkDelta({ attemptId: "a2", delta: "world" }));

    trigger();

    expect(deliver).toHaveBeenCalledTimes(2);
    expect(deliver).toHaveBeenNthCalledWith(1, mkDelta({ delta: "hello" }));
    expect(deliver).toHaveBeenNthCalledWith(
      2,
      mkDelta({ attemptId: "a2", delta: "world" }),
    );
  });

  it("does not coalesce deltas with different conversation ids", () => {
    const deliver = vi.fn();
    const { schedule, trigger } = manualScheduler();
    const coalescer = createChatDeltaCoalescer<TestDelta>(deliver, schedule);

    coalescer.push(mkDelta({ delta: "hello" }));
    coalescer.push(mkDelta({ conversationId: "c2", delta: "world" }));

    trigger();

    expect(deliver).toHaveBeenCalledTimes(2);
  });

  it("does not coalesce deltas across iteration boundary (气泡边界 = 迭代)", () => {
    const deliver = vi.fn();
    const { schedule, trigger } = manualScheduler();
    const coalescer = createChatDeltaCoalescer<TestDelta>(deliver, schedule);

    coalescer.push(mkDelta({ iteration: 1, delta: "first" }));
    coalescer.push(mkDelta({ iteration: 2, delta: "second" }));

    trigger();

    expect(deliver).toHaveBeenCalledTimes(2);
    expect(deliver).toHaveBeenNthCalledWith(
      1,
      mkDelta({ iteration: 1, delta: "first" }),
    );
    expect(deliver).toHaveBeenNthCalledWith(
      2,
      mkDelta({ iteration: 2, delta: "second" }),
    );
  });

  it("flush drains all pending events immediately", () => {
    const deliver = vi.fn();
    const { schedule } = manualScheduler();
    const coalescer = createChatDeltaCoalescer<TestDelta>(deliver, schedule);

    coalescer.push(mkDelta({ delta: "a" }));
    coalescer.push(mkDelta({ delta: "b" }));

    coalescer.flush();

    expect(deliver).toHaveBeenCalledTimes(1);
    expect(deliver).toHaveBeenCalledWith(mkDelta({ delta: "ab" }));
  });

  it("dispose flushes remaining events", () => {
    const deliver = vi.fn();
    const { schedule } = manualScheduler();
    const coalescer = createChatDeltaCoalescer<TestDelta>(deliver, schedule);

    coalescer.push(mkDelta({ delta: "data" }));

    coalescer.dispose();

    expect(deliver).toHaveBeenCalledTimes(1);
  });

  it("handles push after flush correctly", () => {
    const deliver = vi.fn();
    const { schedule, trigger } = manualScheduler();
    const coalescer = createChatDeltaCoalescer<TestDelta>(deliver, schedule);

    coalescer.push(mkDelta({ delta: "first" }));
    coalescer.flush();

    coalescer.push(mkDelta({ delta: "second" }));
    trigger();

    expect(deliver).toHaveBeenCalledTimes(2);
    expect(deliver).toHaveBeenNthCalledWith(1, mkDelta({ delta: "first" }));
    expect(deliver).toHaveBeenNthCalledWith(2, mkDelta({ delta: "second" }));
  });

  it("coalesces across attempt boundary correctly", () => {
    const deliver = vi.fn();
    const { schedule, trigger } = manualScheduler();
    const coalescer = createChatDeltaCoalescer<TestDelta>(deliver, schedule);

    coalescer.push(mkDelta({ delta: "a" }));
    coalescer.push(mkDelta({ delta: "b" }));
    coalescer.push(mkDelta({ attemptId: "a2", delta: "c" }));
    coalescer.push(mkDelta({ attemptId: "a2", delta: "d" }));

    trigger();

    expect(deliver).toHaveBeenCalledTimes(2);
    expect(deliver).toHaveBeenNthCalledWith(1, mkDelta({ delta: "ab" }));
    expect(deliver).toHaveBeenNthCalledWith(
      2,
      mkDelta({ attemptId: "a2", delta: "cd" }),
    );
  });

  it("does nothing on flush with empty queue", () => {
    const deliver = vi.fn();
    const coalescer = createChatDeltaCoalescer<TestDelta>(
      deliver,
      immediateScheduler,
    );

    coalescer.flush();

    expect(deliver).not.toHaveBeenCalled();
  });
});

describe("frameScheduler", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  it("calls flush via setTimeout fallback", () => {
    const flush = vi.fn();
    const cancel = frameScheduler(flush);

    vi.advanceTimersByTime(100);

    expect(flush).toHaveBeenCalledTimes(1);
    cancel();
  });

  it("cancel prevents flush from being called", () => {
    const flush = vi.fn();
    const cancel = frameScheduler(flush);

    cancel();
    vi.advanceTimersByTime(100);

    expect(flush).not.toHaveBeenCalled();
  });
});

describe("createChatDeltaCoalescer with accept", () => {
  it("filters out events rejected by accept", () => {
    const deliver = vi.fn();
    const accept = vi.fn().mockReturnValue(false);
    const coalescer = createChatDeltaCoalescer<TestDelta>(
      deliver,
      immediateScheduler,
      accept,
    );

    coalescer.push(mkDelta({ delta: "dropped" }));

    expect(accept).toHaveBeenCalledTimes(1);
    expect(deliver).not.toHaveBeenCalled();
  });

  it("passes through events accepted by accept", () => {
    const deliver = vi.fn();
    const accept = vi.fn().mockReturnValue(true);
    const coalescer = createChatDeltaCoalescer<TestDelta>(
      deliver,
      immediateScheduler,
      accept,
    );

    coalescer.push(mkDelta({ delta: "kept" }));

    expect(deliver).toHaveBeenCalledTimes(1);
    expect(deliver).toHaveBeenCalledWith(mkDelta({ delta: "kept" }));
  });
});

describe("createSeqDedupFilter", () => {
  it("accepts deltas without seq", () => {
    const { accept } = createSeqDedupFilter();
    expect(accept(mkDelta({ delta: "a" }))).toBe(true);
  });

  it("accepts first delta with seq", () => {
    const { accept } = createSeqDedupFilter();
    expect(accept(mkDelta({ seq: 1, delta: "a" }))).toBe(true);
  });

  it("rejects replayed delta with same or lower seq", () => {
    const { accept } = createSeqDedupFilter();
    expect(accept(mkDelta({ seq: 3, delta: "a" }))).toBe(true);
    expect(accept(mkDelta({ seq: 3, delta: "a" }))).toBe(false);
    expect(accept(mkDelta({ seq: 2, delta: "a" }))).toBe(false);
  });

  it("accepts delta with higher seq", () => {
    const { accept } = createSeqDedupFilter();
    expect(accept(mkDelta({ seq: 1, delta: "a" }))).toBe(true);
    expect(accept(mkDelta({ seq: 2, delta: "b" }))).toBe(true);
    expect(accept(mkDelta({ seq: 3, delta: "c" }))).toBe(true);
  });

  it("tracks seq per conversation+attempt pair", () => {
    const { accept } = createSeqDedupFilter();
    expect(accept(mkDelta({ seq: 1, delta: "a" }))).toBe(true);
    expect(
      accept(mkDelta({ attemptId: "a2", messageId: "m2", seq: 1, delta: "b" })),
    ).toBe(true);
    expect(accept(mkDelta({ seq: 1, delta: "replay" }))).toBe(false);
    expect(
      accept(
        mkDelta({
          attemptId: "a2",
          messageId: "m2",
          seq: 1,
          delta: "replay",
        }),
      ),
    ).toBe(false);
  });

  it("reset clears all tracked seqs", () => {
    const { accept, reset } = createSeqDedupFilter();
    expect(accept(mkDelta({ seq: 1, delta: "a" }))).toBe(true);
    reset();
    expect(accept(mkDelta({ seq: 1, delta: "a" }))).toBe(true);
  });
});
