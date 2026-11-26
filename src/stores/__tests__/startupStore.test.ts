import { describe, it, expect, beforeEach, vi } from "vitest";
import type { CapabilityStateDto } from "@/types/protocol.contract";

const invokeMock = vi.hoisted(() => {
  const calls: string[] = [];
  const state = {
    snapshot: [] as CapabilityStateDto[],
    failure: null as Error | null,
  };
  return {
    calls,
    state,
    tauriInvoke: (command: string): Promise<unknown> => {
      calls.push(command);
      if (state.failure) return Promise.reject(state.failure);
      return Promise.resolve(state.snapshot);
    },
  };
});

vi.mock("@/services/system/tauriInvoke", () => ({
  tauriInvoke: invokeMock.tauriInvoke,
}));

const listenMock = vi.hoisted(() => {
  const state = {
    handlers: new Map<string, (event: { payload: unknown }) => void>(),
    resolve: null as null | ((unlisten: () => void) => void),
    reject: null as null | ((error: Error) => void),
  };
  const listen = (
    name: string,
    handler: (event: { payload: unknown }) => void,
  ) => {
    state.handlers.set(name, handler);
    return new Promise<() => void>((resolve, reject) => {
      state.resolve = resolve;
      state.reject = reject;
    });
  };
  return { state, listen };
});

vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock.listen }));

import {
  subscribeStartupCapabilities,
  useStartupStore,
} from "../system/startupStore";

const EVENT = "startup:capability-changed";

function pending(): CapabilityStateDto {
  return { id: "tool_runtime", state: "pending", message: null };
}

function ready(): CapabilityStateDto {
  return { id: "tool_runtime", state: "ready", message: null };
}

/** 让 `listen().then` / `hydrate` 的微任务与宏任务都跑完。 */
function flush(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

describe("startupStore 能力状态水合（I75 §4.4 C4 / C5 / C7）", () => {
  beforeEach(() => {
    invokeMock.calls.length = 0;
    invokeMock.state.snapshot = [];
    invokeMock.state.failure = null;
    listenMock.state.handlers.clear();
    listenMock.state.resolve = null;
    listenMock.state.reject = null;
    useStartupStore.setState({ states: {}, hydrated: false });
  });

  it("先注册监听，注册成功后才拉快照", async () => {
    subscribeStartupCapabilities();
    expect(invokeMock.calls).toEqual([]);

    listenMock.state.resolve?.(() => undefined);
    await flush();

    expect(invokeMock.calls).toEqual(["get_capability_states"]);
  });

  it("监听不可用时仍水合，不让状态停在未水合", async () => {
    subscribeStartupCapabilities();
    listenMock.state.reject?.(new Error("no ipc"));
    await flush();

    expect(invokeMock.calls).toEqual(["get_capability_states"]);
    expect(useStartupStore.getState().hydrated).toBe(true);
  });

  it("晚到的快照不覆盖已收到的新状态", async () => {
    invokeMock.state.snapshot = [pending()];
    useStartupStore.getState().applyState(ready());

    await useStartupStore.getState().hydrate();

    expect(useStartupStore.getState().states.tool_runtime?.state).toBe("ready");
  });

  it("快照补齐事件未覆盖的条目", async () => {
    invokeMock.state.snapshot = [pending()];

    await useStartupStore.getState().hydrate();

    expect(useStartupStore.getState().states.tool_runtime?.state).toBe(
      "pending",
    );
    expect(useStartupStore.getState().hydrated).toBe(true);
  });

  it("事件到达即视为已水合：快照失败时提示仍会出现", async () => {
    invokeMock.state.failure = new Error("snapshot failed");
    await useStartupStore.getState().hydrate();
    expect(useStartupStore.getState().hydrated).toBe(false);

    useStartupStore.getState().applyState(ready());

    expect(useStartupStore.getState().hydrated).toBe(true);
    expect(useStartupStore.getState().states.tool_runtime?.state).toBe("ready");
  });

  it("订阅后到达的 ready 事件能落到 store（Orb 窗口的常驻 pending 场景）", async () => {
    invokeMock.state.snapshot = [pending()];
    subscribeStartupCapabilities();
    listenMock.state.resolve?.(() => undefined);
    await flush();
    expect(useStartupStore.getState().states.tool_runtime?.state).toBe(
      "pending",
    );

    const handler = listenMock.state.handlers.get(EVENT);
    expect(handler).toBeDefined();
    handler?.({ payload: ready() });

    expect(useStartupStore.getState().states.tool_runtime?.state).toBe("ready");
  });

  it("解绑后事件不再写入", async () => {
    invokeMock.state.snapshot = [pending()];
    const dispose = subscribeStartupCapabilities();
    listenMock.state.resolve?.(() => undefined);
    await flush();
    dispose();

    listenMock.state.handlers.get(EVENT)?.({ payload: ready() });

    expect(useStartupStore.getState().states.tool_runtime?.state).toBe(
      "pending",
    );
  });
});
