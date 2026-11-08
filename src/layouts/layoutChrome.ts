import type { ComponentType } from "react";

export interface LayoutChromeProps {
  onToggleMode: () => void;
}

/** 两套外壳只分叉标题栏与侧栏；路由内容由 AppShell 稳定持有。 */
export interface LayoutChrome {
  Titlebar: ComponentType<LayoutChromeProps>;
  Sidebar: ComponentType | null;
}
