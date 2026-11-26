import { create } from "zustand";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { CapabilityStateDto } from "@/types/protocol.contract";
import { tauriInvoke } from "@/services/system/tauriInvoke";
import { createLogger } from "@/services/system/logger";

const log = createLogger("startupStore");

/** 与后端 `protocol::events::event_name::STARTUP_CAPABILITY_CHANGED` 同源（I75 §4.4 C4）。 */
const CAPABILITY_CHANGED_EVENT = "startup:capability-changed";

interface StartupState {
  /** 能力状态按 id 索引；未水合前为空表，不臆造 `Pending`。 */
  states: Record<string, CapabilityStateDto>;
  hydrated: boolean;
  hydrate: () => Promise<void>;
  applyState: (payload: CapabilityStateDto) => void;
  /** 重试入口（C5）：只触发后端 `ensure`，结果经事件回流。 */
  retry: (id: string) => Promise<void>;
}

/**
 * 启动能力状态（I75 §4.4 C5）。
 *
 * 首帧后拉一次全表快照水合，之后只靠 `startup:capability-changed` 增量刷新，不轮询。
 * 只覆盖 `ACTIONABLE` 集合成员（当前仅 `tool_runtime`），其余降档项无 UI。
 */
export const useStartupStore = create<StartupState>((set) => ({
  states: {},
  hydrated: false,

  hydrate: async () => {
    try {
      const states = await tauriInvoke<CapabilityStateDto[]>(
        "get_capability_states",
      );
      set({
        states: Object.fromEntries(states.map((item) => [item.id, item])),
        hydrated: true,
      });
    } catch (e) {
      log.debug("capability states unavailable", e);
    }
  },

  applyState: (payload) => {
    set((state) => ({
      states: { ...state.states, [payload.id]: payload },
    }));
  },

  retry: async (id) => {
    try {
      await tauriInvoke("retry_capability", { id });
    } catch (e) {
      log.warn("retry capability failed", { id, error: e });
    }
  },
}));

/** 订阅能力状态变更；返回解绑函数（与 `subscribeSettingEvents` 同一约定）。 */
export function subscribeStartupCapabilities(): () => void {
  let disposed = false;
  const unlisteners: UnlistenFn[] = [];

  void useStartupStore.getState().hydrate();

  listen<CapabilityStateDto>(CAPABILITY_CHANGED_EVENT, ({ payload }) => {
    if (disposed) return;
    useStartupStore.getState().applyState(payload);
  })
    .then((unlisten) => {
      if (disposed) unlisten();
      else unlisteners.push(unlisten);
    })
    .catch((error) => {
      log.debug("capability listener unavailable", error);
    });

  return () => {
    disposed = true;
    for (const unlisten of unlisteners.splice(0)) unlisten();
  };
}
