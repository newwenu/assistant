import { Titlebar } from "@/components/Titlebar";
import type { LayoutChrome, LayoutChromeProps } from "./layoutChrome";

function MiniTitlebar({ onToggleMode }: LayoutChromeProps) {
  return <Titlebar mode="mini" onToggleMode={onToggleMode} />;
}

export const MiniLayout: LayoutChrome = {
  Titlebar: MiniTitlebar,
  Sidebar: null,
};
