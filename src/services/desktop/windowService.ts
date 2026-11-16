import { tauriInvoke as invoke } from "../system/tauriInvoke";

export type WindowMode = "normal" | "mini";

export const windowService = {
  setMainWindowMode: (mode: WindowMode): Promise<void> =>
    invoke("set_main_window_mode", { mode }),
};
