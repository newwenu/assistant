import { listen } from "@tauri-apps/api/event";
import type { AgentNodeEvent } from "../chat/core/chatTypes";
import { ChatEvent } from "../chat/core/chatTypes";
import { useObservationStore } from "../stores/agent/observation/store";
import { createLogger } from "./system/logger";

const log = createLogger("observationEvents");

let started = false;

/**
 * 主窗口侧的观测订阅（I55 §6.3 第 1 条：节点事件不做活跃会话过滤）。
 *
 * 为什么主窗口要单独订阅：`agent-node-event` 走 `EventRoute::Broadcast`，
 * 但既有的 `subscribeChatStreamEvents` 只在 orb 窗口建立（`OrbPanel`），
 * 观测页在**主窗口**，所以主窗口此前收不到任何节点事件，实例恒为 0。
 *
 * 这条订阅只写观测存储，不接编排层、不碰控制路径（O6 / §6.5）；
 * orb 窗口不调用它（`OrbWindow` 不经过 `useAppInitialization`），
 * 两个窗口各自持有独立 store，不存在重复折叠。
 *
 * 幂等：重复调用不会重复折叠（同一事件折叠两次会污染 timeline）。
 */
export function ensureObservationEvents(): void {
  if (started) return;
  started = true;
  void listen<AgentNodeEvent>(ChatEvent.AgentNodeEvent, (event) => {
    useObservationStore.getState().applyNodeEvent(event.payload);
  }).catch((err: unknown) => {
    started = false;
    log.error("Failed to subscribe agent-node-event", err);
  });
}
