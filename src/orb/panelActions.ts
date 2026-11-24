import { getCurrentWebview } from "@tauri-apps/api/webview";
import { openAttachmentFile } from "@/chat/attachment/paths";
import { workspaceService } from "../services/desktop/workspaceService";
import { WINDOW_ORB } from "@/config/constants";
import { orbService } from "../services/desktop/orbService";
import { createLogger } from "@/services/system/logger";
import { notifyError } from "@/stores/system/notifyStore";
import { useOrbStore } from "../stores/orbStore";
import { collapseOrbPanel, suppressOrbBlurCollapse } from "./OrbExpand";

const log = createLogger("PanelActions");

export type PanelOpenTarget =
  | { kind: "attachment"; id: string }
  | { kind: "workspace"; conversationId: string }
  | { kind: "main"; route?: string };

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

  let opened = false;
  try {
    opened = await runTarget(target);
  } catch (e) {
    log.error("openFromPanel failed", { kind: target.kind, error: e });
    if (target.kind === "attachment") {
      notifyError(
        "chat.attachment.open_failed_title",
        "chat.attachment.open_failed_description",
      );
    } else if (target.kind === "workspace") {
      notifyError(
        "workspace.error.open.title",
        "workspace.error.open.description",
      );
    }
  } finally {
    if (shouldCollapse && opened) {
      await collapseOrbPanel();
    }
  }
}

async function runTarget(target: PanelOpenTarget): Promise<boolean> {
  switch (target.kind) {
    case "attachment": {
      const opened = await openAttachmentFile(target.id);
      if (!opened) {
        notifyError(
          "chat.attachment.missing_title",
          "chat.attachment.missing_description",
        );
      }
      return opened;
    }
    case "workspace": {
      // 路径由后端 `resolve_workspace` 推导，前端只传会话 id（见 `open_workspace_dir`）。
      const opened = await workspaceService.openDir(target.conversationId);
      if (!opened) {
        notifyError(
          "workspace.error.open.title",
          "workspace.error.open.description",
        );
      }
      return opened;
    }
    case "main":
      if (target.route) {
        await orbService.openMainWindowAndNavigate(target.route);
      } else {
        await orbService.openMainWindow();
      }
      return true;
  }
}
