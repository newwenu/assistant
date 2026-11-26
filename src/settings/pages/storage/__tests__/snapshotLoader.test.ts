import { describe, expect, it, vi } from "vitest";
import type { UnlistenFn } from "@tauri-apps/api/event";
import {
  STORAGE_ENTRY_STATS_UPDATED,
  STORAGE_INVENTORY_UPDATED,
  createSnapshotLoader,
  subscribeSnapshotEvents,
  type SnapshotPhase,
} from "../useStorageSnapshot";

interface Deferred<T> {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (reason: unknown) => void;
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const flush = (): Promise<void> =>
  new Promise<void>((resolve) => {
    setTimeout(resolve, 0);
  });

function harness(options: { alive?: () => boolean } = {}) {
  const calls: Deferred<string>[] = [];
  const fetch = vi.fn((): Promise<string> => {
    const next = deferred<string>();
    calls.push(next);
    return next.promise;
  });
  const phases: SnapshotPhase[] = [];
  const data: string[] = [];
  const errors: string[] = [];
  const loader = createSnapshotLoader(fetch, {
    onPhase: (phase) => phases.push(phase),
    onData: (value) => data.push(value),
    onError: (message) => errors.push(message),
    isAlive: options.alive ?? (() => true),
  });
  return { loader, fetch, calls, phases, data, errors };
}

describe("存储快照事件订阅", () => {
  function source() {
    const subscribed: string[] = [];
    const handlers: Array<() => void> = [];
    const unlisten = vi.fn();
    const listen = vi.fn(
      async (event: string, handler: () => void): Promise<UnlistenFn> => {
        subscribed.push(event);
        handlers.push(handler);
        return unlisten;
      },
    );
    return { listen, subscribed, handlers, unlisten };
  }

  it("两个事件都订阅，任一到达都触发一次整页重拉", async () => {
    const s = source();
    const onEvent = vi.fn();
    subscribeSnapshotEvents(s, onEvent);
    await flush();

    expect(s.subscribed).toEqual([
      STORAGE_INVENTORY_UPDATED,
      STORAGE_ENTRY_STATS_UPDATED,
    ]);

    s.handlers[0]();
    expect(onEvent).toHaveBeenCalledTimes(1);
    s.handlers[1]();
    expect(onEvent).toHaveBeenCalledTimes(2);
  });

  it("退订后两个事件的监听都被移除", async () => {
    const s = source();
    const dispose = subscribeSnapshotEvents(s, vi.fn());
    await flush();
    dispose();
    expect(s.unlisten).toHaveBeenCalledTimes(2);
  });

  it("退订先于订阅就绪时就绪即退订，不留下监听", async () => {
    const s = source();
    const dispose = subscribeSnapshotEvents(s, vi.fn());
    dispose();
    await flush();
    expect(s.unlisten).toHaveBeenCalledTimes(2);
  });
});

describe("存储快照拉取协调", () => {
  it("初始拉取：进入 initial，成功后 data 回写并转 ready", async () => {
    const h = harness();
    const task = h.loader.load("initial");
    expect(h.fetch).toHaveBeenCalledTimes(1);
    expect(h.phases).toEqual(["initial"]);

    h.calls[0].resolve("first");
    await task;

    expect(h.data).toEqual(["first"]);
    expect(h.phases).toEqual(["initial", "ready"]);
    expect(h.errors).toEqual([]);
  });

  it("在飞时事件到达：结束后补一次刷新（不丢弃）", async () => {
    const h = harness();
    const task = h.loader.load("initial");
    expect(h.fetch).toHaveBeenCalledTimes(1);

    void h.loader.load("refresh");
    expect(h.fetch).toHaveBeenCalledTimes(1);

    h.calls[0].resolve("first");
    await flush();

    expect(h.fetch).toHaveBeenCalledTimes(2);
    expect(h.phases).toEqual(["initial", "ready", "refreshing"]);

    h.calls[1].resolve("second");
    await task;

    expect(h.data).toEqual(["first", "second"]);
    expect(h.phases).toEqual(["initial", "ready", "refreshing", "ready"]);
  });

  it("尾随期间多个事件合并成一次补拉，不叠加", async () => {
    const h = harness();
    const task = h.loader.load("refresh");

    void h.loader.load("refresh");
    void h.loader.load("refresh");
    void h.loader.load("refresh");
    expect(h.fetch).toHaveBeenCalledTimes(1);

    h.calls[0].resolve("first");
    await flush();
    expect(h.fetch).toHaveBeenCalledTimes(2);

    h.calls[1].resolve("second");
    await flush();
    expect(h.fetch).toHaveBeenCalledTimes(2);
    expect(h.data).toEqual(["first", "second"]);
    await task;
  });

  it("一轮结束后 inFlight 归零，后续事件开启新的一轮", async () => {
    const h = harness();
    const task = h.loader.load("refresh");
    h.calls[0].resolve("first");
    await task;
    expect(h.fetch).toHaveBeenCalledTimes(1);

    void h.loader.load("refresh");
    expect(h.fetch).toHaveBeenCalledTimes(2);

    h.calls[1].resolve("second");
    await flush();
    expect(h.data).toEqual(["first", "second"]);
  });

  it("首次失败：错误上报且回到 initial（可重试）", async () => {
    const h = harness();
    const task = h.loader.load("initial");

    h.calls[0].reject(new Error("boom"));
    await task;

    expect(h.errors).toEqual(["boom"]);
    expect(h.phases).toEqual(["initial", "initial"]);
    expect(h.data).toEqual([]);
  });

  it("已有数据时刷新失败：保留数据并回到 ready", async () => {
    const h = harness();
    const first = h.loader.load("initial");
    h.calls[0].resolve("first");
    await first;

    const second = h.loader.load("refresh");
    h.calls[1].reject(new Error("boom"));
    await second;

    expect(h.data).toEqual(["first"]);
    expect(h.errors).toEqual(["boom"]);
    expect(h.phases).toEqual(["initial", "ready", "refreshing", "ready"]);
  });

  it("非 Error 抛出的失败也转成字符串", async () => {
    const h = harness();
    const task = h.loader.load("refresh");
    h.calls[0].reject("plain");
    await task;
    expect(h.errors).toEqual(["plain"]);
  });

  it("卸载后（isAlive=false）不再回写任何状态", async () => {
    let alive = true;
    const h = harness({ alive: () => alive });
    const task = h.loader.load("initial");
    alive = false;

    h.calls[0].resolve("late");
    await task;

    expect(h.data).toEqual([]);
    expect(h.phases).toEqual(["initial"]);
  });
});
