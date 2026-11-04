import { AppSidebar } from "@/components/AppSidebar";
import { Titlebar } from "@/components/Titlebar";
import type { LayoutChrome, LayoutChromeProps } from "./layoutChrome";

function NormalTitlebar({ onToggleMode }: LayoutChromeProps) {
  return <Titlebar mode="normal" onToggleMode={onToggleMode} />;
}

export const NormalLayout: LayoutChrome = {
  Titlebar: NormalTitlebar,
  Sidebar: AppSidebar,
};
