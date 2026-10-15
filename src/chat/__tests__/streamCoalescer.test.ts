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

    coalescer.push({
      conversation_id: "c1",
      message_id: "m1",
      delta: "hello",
    });

    expect(deliver).toHaveBeenCalledTimes(1);
    expect(deliver).toHaveBeenCalledWith({
      conversation_id: "c1",
      message_id: "m1",
      delta: "hello",
    });
  });

  it("coalesces consecutive deltas with same conversation and message id", () => {
    const deliver = vi.fn();
    const { schedule, trigger } = manualScheduler();
    const coalescer = createChatDeltaCoalescer<TestDelta>(deliver, schedule);

    coalescer.push({
      conversation_id: "c1",
      message_id: "m1",
      delta: "hel",
    });
    coalescer.push({
      conversation_id: "c1",
      message_id: "m1",
      delta: "lo",
    });
    coalescer.push({
      conversation_id: "c1",
      message_id: "m1",
      delta: " world",
    });

    trigger();

    expect(deliver).toHaveBeenCalledTimes(1);
    expect(deliver).toHaveBeenCalledWith({
      conversation_id: "c1",
      message_id: "m1",
      delta: "hello world",
    });
  });

  it("does not coalesce deltas with different message ids", () => {
    const deliver = vi.fn();
    const { schedule, trigger } = manualScheduler();
    const coalescer = createChatDeltaCoalescer<TestDelta>(deliver, schedule);

    coalescer.push({
      conversation_id: "c1",
      message_id: "m1",
      delta: "hello",
    });
    coalescer.push({
      conversation_id: "c1",
      message_id: "m2",
      delta: "world",
    });

    trigger();

    expect(deliver).toHaveBeenCalledTimes(2);
    expect(deliver).toHaveBeenNthCalledWith(1, {
      conversation_id: "c1",
      message_id: "m1",
      delta: "hello",
    });
    expect(deliver).toHaveBeenNthCalledWith(2, {
      conversation_id: "c1",
      message_id: "m2",
      delta: "world",
    });
  });

  it("does not coalesce deltas with different conversation ids", () => {
    const deliver = vi.fn();
    const { schedule, trigger } = manualScheduler();
    const coalescer = createChatDeltaCoalescer<TestDelta>(deliver, schedule);

    coalescer.push({
      conversation_id: "c1",
      message_id: "m1",
      delta: "hello",
    });
    coalescer.push({
      conversation_id: "c2",
      message_id: "m1",
      delta: "world",
    });

    trigger();

    expect(deliver).toHaveBeenCalledTimes(2);
  });

  it("flush drains all pending events immediately", () => {
    const deliver = vi.fn();
    const { schedule } = manualScheduler();
    const coalescer = createChatDeltaCoalescer<TestDelta>(deliver, schedule);

    coalescer.push({
      conversation_id: "c1",
      message_id: "m1",
      delta: "a",
    });
    coalescer.push({
      conversation_id: "c1",
      message_id: "m1",
      delta: "b",
    });

    coalescer.flush();

    expect(deliver).toHaveBeenCalledTimes(1);
    expect(deliver).toHaveBeenCalledWith({
      conversation_id: "c1",
      message_id: "m1",
      delta: "ab",
    });
  });

  it("dispose flushes remaining events", () => {
    const deliver = vi.fn();
    const { schedule } = manualScheduler();
    const coalescer = createChatDeltaCoalescer<TestDelta>(deliver, schedule);

    coalescer.push({
      conversation_id: "c1",
      message_id: "m1",
      delta: "data",
    });

    coalescer.dispose();

    expect(deliver).toHaveBeenCalledTimes(1);
  });

  it("handles push after flush correctly", () => {
    const deliver = vi.fn();
    const { schedule, trigger } = manualScheduler();
    const coalescer = createChatDeltaCoalescer<TestDelta>(deliver, schedule);

    coalescer.push({
      conversation_id: "c1",
      message_id: "m1",
      delta: "first",
    });
    coalescer.flush();

    coalescer.push({
      conversation_id: "c1",
      message_id: "m1",
      delta: "second",
    });
    trigger();

    expect(deliver).toHaveBeenCalledTimes(2);
    expect(deliver).toHaveBeenNthCalledWith(1, {
      conversation_id: "c1",
      message_id: "m1",
      delta: "first",
    });
    expect(deliver).toHaveBeenNthCalledWith(2, {
      conversation_id: "c1",
      message_id: "m1",
      delta: "second",
    });
  });

  it("coalesces across message boundary correctly", () => {
    const deliver = vi.fn();
    const { schedule, trigger } = manualScheduler();
    const coalescer = createChatDeltaCoalescer<TestDelta>(deliver, schedule);

    coalescer.push({
      conversation_id: "c1",
      message_id: "m1",
      delta: "a",
    });
    coalescer.push({
      conversation_id: "c1",
      message_id: "m1",
      delta: "b",
    });
    coalescer.push({
      conversation_id: "c1",
      message_id: "m2",
      delta: "c",
    });
    coalescer.push({
      conversation_id: "c1",
      message_id: "m2",
      delta: "d",
    });

    trigger();

    expect(deliver).toHaveBeenCalledTimes(2);
    expect(deliver).toHaveBeenNthCalledWith(1, {
      conversation_id: "c1",
      message_id: "m1",
      delta: "ab",
    });
    expect(deliver).toHaveBeenNthCalledWith(2, {
      conversation_id: "c1",
      message_id: "m2",
      delta: "cd",
    });
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

    coalescer.push({
      conversation_id: "c1",
      message_id: "m1",
      delta: "dropped",
    });

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

    coalescer.push({
      conversation_id: "c1",
      message_id: "m1",
      delta: "kept",
    });

    expect(deliver).toHaveBeenCalledTimes(1);
    expect(deliver).toHaveBeenCalledWith({
      conversation_id: "c1",
      message_id: "m1",
      delta: "kept",
    });
  });
});

describe("createSeqDedupFilter", () => {
  it("accepts deltas without seq", () => {
    const { accept } = createSeqDedupFilter();
    expect(
      accept({
        conversation_id: "c1",
        message_id: "m1",
        delta: "a",
      }),
    ).toBe(true);
  });

  it("accepts first delta with seq", () => {
    const { accept } = createSeqDedupFilter();
    expect(
      accept({
        conversation_id: "c1",
        message_id: "m1",
        seq: 1,
        delta: "a",
      }),
    ).toBe(true);
  });

  it("rejects replayed delta with same or lower seq", () => {
    const { accept } = createSeqDedupFilter();
    expect(
      accept({
        conversation_id: "c1",
        message_id: "m1",
        seq: 3,
        delta: "a",
      }),
    ).toBe(true);
    expect(
      accept({
        conversation_id: "c1",
        message_id: "m1",
        seq: 3,
        delta: "a",
      }),
    ).toBe(false);
    expect(
      accept({
        conversation_id: "c1",
        message_id: "m1",
        seq: 2,
        delta: "a",
      }),
    ).toBe(false);
  });

  it("accepts delta with higher seq", () => {
    const { accept } = createSeqDedupFilter();
    expect(
      accept({
        conversation_id: "c1",
        message_id: "m1",
        seq: 1,
        delta: "a",
      }),
    ).toBe(true);
    expect(
      accept({
        conversation_id: "c1",
        message_id: "m1",
        seq: 2,
        delta: "b",
      }),
    ).toBe(true);
    expect(
      accept({
        conversation_id: "c1",
        message_id: "m1",
        seq: 3,
        delta: "c",
      }),
    ).toBe(true);
  });

  it("tracks seq per conversation+message pair", () => {
    const { accept } = createSeqDedupFilter();
    expect(
      accept({
        conversation_id: "c1",
        message_id: "m1",
        seq: 1,
        delta: "a",
      }),
    ).toBe(true);
    expect(
      accept({
        conversation_id: "c1",
        message_id: "m2",
        seq: 1,
        delta: "b",
      }),
    ).toBe(true);
    expect(
      accept({
        conversation_id: "c1",
        message_id: "m1",
        seq: 1,
        delta: "replay",
      }),
    ).toBe(false);
    expect(
      accept({
        conversation_id: "c1",
        message_id: "m2",
        seq: 1,
        delta: "replay",
      }),
    ).toBe(false);
  });

  it("reset clears all tracked seqs", () => {
    const { accept, reset } = createSeqDedupFilter();
    expect(
      accept({
        conversation_id: "c1",
        message_id: "m1",
        seq: 1,
        delta: "a",
      }),
    ).toBe(true);
    reset();
    expect(
      accept({
        conversation_id: "c1",
        message_id: "m1",
        seq: 1,
        delta: "a",
      }),
    ).toBe(true);
  });
});
