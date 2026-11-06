import type {
  MousePositionDto,
  ScreenSizeDto,
} from "@/types/protocol.contract";
import { tauriInvoke as invoke } from "./tauriInvoke";

export const orbService = {
  getMousePosition: (): Promise<MousePositionDto | null> =>
    invoke<MousePositionDto | null>("get_mouse_position"),

  getScreenSize: (
    centerX: number,
    centerY: number,
    scaleFactor: number,
  ): Promise<ScreenSizeDto> =>
    invoke<ScreenSizeDto>("get_screen_size", {
      centerX,
      centerY,
      scaleFactor,
    }),

  setOrbClickThrough: (enabled: boolean): Promise<void> =>
    invoke("set_orb_click_through", { enabled }),

  expandOrbPanel: (horizontal: string, vertical: string): Promise<void> =>
    invoke("expand_orb_panel", { horizontal, vertical }),

  collapseOrbPanel: (horizontal: string, vertical: string): Promise<void> =>
    invoke("collapse_orb_panel", { horizontal, vertical }),

  snapOrbToEdge: (): Promise<void> => invoke("snap_orb_to_edge"),

  showOrbContextMenu: (x: number, y: number): Promise<void> =>
    invoke("show_orb_context_menu", { x, y }),

  openMainWindowAndNavigate: (route: string): Promise<void> =>
    invoke("open_main_window_and_navigate", { route }),

  openMainWindow: (): Promise<void> => invoke("open_main_window"),

  toggleOrbVisibility: (): Promise<void> => invoke("toggle_orb_visibility"),

  snapOrbToScreen: (): Promise<void> => invoke("snap_orb_to_screen"),
};
