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
      const snapshot = Object.fromEntries(
        states.map((item) => [item.id, item]),
      );
      set((state) => ({
        // 快照只填缺口：订阅先于快照注册（见 `subscribeStartupCapabilities`），
        // 因此已落地的条目一定晚于快照生成时刻，不能被快照覆盖回 `pending`。
        states: { ...snapshot, ...state.states },
        hydrated: true,
      }));
    } catch (e) {
      log.debug("capability states unavailable", e);
    }
  },

  applyState: (payload) => {
    set((state) => ({
      states: { ...state.states, [payload.id]: payload },
      // 收到真实迁移即已水合：快照那次 IPC 失败时也不能让提示永不出现（C7 ③）。
      hydrated: true,
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

  // 顺序即正确性：事件不重放，注册前发出的迁移永久丢失。Orb 窗口在 Boot 阶段
  // 就完成水合，而 `tool_runtime` 在首帧后数毫秒即 `pending → ready`——先拉快照
  // 只会拿到 `pending` 且再也等不到事件，表现为「正在准备」常驻。
  listen<CapabilityStateDto>(CAPABILITY_CHANGED_EVENT, ({ payload }) => {
    if (disposed) return;
    useStartupStore.getState().applyState(payload);
  })
    .then((unlisten) => {
      if (disposed) {
        unlisten();
        return;
      }
      unlisteners.push(unlisten);
      void useStartupStore.getState().hydrate();
    })
    .catch((error) => {
      log.debug("capability listener unavailable", error);
      // 监听不可用时仍要水合：否则 `hydrated` 恒为 false，连失败提示都不出。
      if (!disposed) void useStartupStore.getState().hydrate();
    });

  return () => {
    disposed = true;
    for (const unlisten of unlisteners.splice(0)) unlisten();
  };
}
