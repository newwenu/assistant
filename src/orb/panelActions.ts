import { getCurrentWebview } from "@tauri-apps/api/webview";
import { openAttachmentFile } from "@/chat/attachment/paths";
import { WINDOW_ORB } from "@/config/constants";
import { orbService } from "../services/orbService";
import { createLogger } from "@/services/logger";
import { useOrbStore } from "../stores/orbStore";
import { collapseOrbPanel, suppressOrbBlurCollapse } from "./OrbExpand";

const log = createLogger("PanelActions");

export type PanelOpenTarget =
  { kind: "attachment"; id: string } | { kind: "main"; route?: string };

export type PanelCollapsePolicy = "always" | "never";

/**
 * 面板内所有“打开”动作的唯一入口。
 * 打开会抢走 orb 窗口焦点，因此收起由这里显式决定，不再依赖失焦启发式。
 */
export async function openFromPanel(
  target: PanelOpenTarget,
  policy: PanelCollapsePolicy = "always",
): Promise<void> {
  const inOrbWindow = getCurrentWebview().label === WINDOW_ORB;
  const shouldCollapse =
    inOrbWindow && policy === "always" && !useOrbStore.getState().keepExpanded;

  if (shouldCollapse) {
    suppressOrbBlurCollapse();
  }

  try {
    await runTarget(target);
  } catch (e) {
    log.error("openFromPanel failed", { kind: target.kind, error: e });
  } finally {
    if (shouldCollapse) {
      await collapseOrbPanel();
    }
  }
}

async function runTarget(target: PanelOpenTarget): Promise<void> {
  switch (target.kind) {
    case "attachment":
      await openAttachmentFile(target.id);
      return;
    case "main":
      if (target.route) {
        await orbService.openMainWindowAndNavigate(target.route);
      } else {
        await orbService.openMainWindow();
      }
      return;
  }
}
