import type { NavigateFunction } from "react-router-dom";
import {
  useWindowModeStore,
  type WindowMode,
} from "@/stores/system/windowModeStore";
import { toolsMap } from "./registry";

function pathnameOf(path: string): string {
  return path.split("?")[0];
}

function toolIdOf(pathname: string): string | null {
  if (!pathname.startsWith("/tools/")) return null;
  const id = pathname.slice("/tools/".length);
  return id && !id.includes("/") ? id : null;
}

/**
 * 某条路由允许的最小形态：
 * - `/` 与 miniFriendly 工具页允许 mini；
 * - 工具页不兼容、设置、历史与特性路由都要求 normal。
 */
export function resolveNavigateMode(path: string): WindowMode {
  const pathname = pathnameOf(path);
  if (pathname === "/") return "mini";
  const toolId = toolIdOf(pathname);
  if (toolId && toolsMap[toolId]?.miniFriendly === true) return "mini";
  return "normal";
}

/** 从当前形态导航到目标路由；需要时先展开窗口。 */
export async function navigateInCurrentMode(
  navigate: NavigateFunction,
  path: string,
): Promise<void> {
  const state = useWindowModeStore.getState();
  if (state.mode === "mini" && resolveNavigateMode(path) === "normal") {
    await state.setMode("normal");
  }
  await navigate(path);
}

/** 工具列表点击的统一入口。 */
export async function openTool(
  navigate: NavigateFunction,
  toolId: string,
): Promise<void> {
  await navigateInCurrentMode(navigate, `/tools/${toolId}`);
}
