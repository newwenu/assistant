import { create } from "zustand";
import type { ScreenshotSessionDto } from "@/types/protocol.contract";

interface ScreenshotState {
  session: ScreenshotSessionDto | null;
  /** 工具栏入口：点击后立刻隐藏聊天 UI，抓屏完成或失败后复位。 */
  preparing: boolean;
  /** 已发出 `crop`/`cancel` 但 Rust 尚未恢复几何：覆盖层只能渲染中性底色。 */
  finishing: boolean;
  setSession: (session: ScreenshotSessionDto | null) => void;
  clearSession: () => void;
  beginPreparing: () => void;
  endPreparing: () => void;
  beginFinishing: () => void;
  endFinishing: () => void;
}

export const useScreenshotStore = create<ScreenshotState>((set) => ({
  session: null,
  preparing: false,
  finishing: false,

  setSession: (session) => set({ session, preparing: false }),
  clearSession: () =>
    set({ session: null, finishing: false, preparing: false }),
  beginPreparing: () => set({ preparing: true }),
  endPreparing: () => set({ preparing: false }),
  beginFinishing: () => set({ finishing: true }),
  endFinishing: () => set({ finishing: false }),
}));
