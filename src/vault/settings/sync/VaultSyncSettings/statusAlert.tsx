import type { ReactNode } from "react";
import { Button } from "antd";
import type { TFunction } from "i18next";

import type { SyncDisplay } from "../../../core/display/syncDisplay";
import { remoteErrorText } from "../../../core/display/syncDisplay";
import type { RemoteStatus, VaultErrorCode } from "../../../core/vault.types";

import { syncErrorText } from "./syncResult";

export interface SyncAlert {
  type: "success" | "info" | "warning" | "error";
  title: ReactNode;
  description?: ReactNode;
  action?: ReactNode;
}

interface SyncAlertInput {
  display: SyncDisplay;
  t: TFunction;
  errorCode: VaultErrorCode | null;
  remoteStatus: RemoteStatus | null;
  checkRemoteAction: ReactNode;
  onAbort: () => void;
  onFocusRemoteUrl: () => void;
}

/**
 * 就绪度 → 顶部状态告警。每种 `SyncDisplay` 分支对应一句固定文案，
 * 可操作分支（仓库异常 / 未绑远端 / 各种领先落后）挂上对应的动作按钮。
 */
export function buildSyncAlert({
  display,
  t,
  errorCode,
  remoteStatus,
  checkRemoteAction,
  onAbort,
  onFocusRemoteUrl,
}: SyncAlertInput): SyncAlert {
  switch (display.kind) {
    case "unknown":
      return { type: "info", title: t("vault.sync.status_unknown") };
    case "failed":
      return {
        type: "error",
        title: syncErrorText(errorCode, display.message),
      };
    case "repoState":
      return {
        type: "warning",
        title: t("vault.sync.repo_state_blocked", { state: display.state }),
        action: (
          <Button size="small" onClick={onAbort}>
            {t("vault.sync.abort")}
          </Button>
        ),
      };
    case "noSync":
      return { type: "info", title: t("vault.sync.status_no_sync") };
    case "noRemote":
      return {
        type: "warning",
        title: t("vault.sync.status_no_remote"),
        description: t("vault.sync.remote_not_bound_tip"),
        action: (
          <Button size="small" onClick={onFocusRemoteUrl}>
            {t("vault.sync.remote_not_bound_cta")}
          </Button>
        ),
      };
    case "dirty":
      return {
        type: "warning",
        title: t("vault.sync.status_dirty"),
        action: checkRemoteAction,
      };
    case "ahead":
      return {
        type: "warning",
        title: t("vault.sync.state_ahead", { count: display.ahead }),
        action: checkRemoteAction,
      };
    case "behind":
      return {
        type: "warning",
        title: t("vault.sync.state_behind", { count: display.behind }),
        action: checkRemoteAction,
      };
    case "diverged":
      return {
        type: "warning",
        title: t("vault.sync.state_diverged", {
          ahead: display.ahead,
          behind: display.behind,
        }),
        action: checkRemoteAction,
      };
    case "inSync": {
      const time = new Date(display.checkedAt).toLocaleString();
      return {
        type: "success",
        title: t("vault.sync.state_in_sync"),
        description: display.fetched
          ? t("vault.sync.remote_checked_at", { time })
          : t("vault.sync.remote_checked_stale", { time }),
        action: checkRemoteAction,
      };
    }
    case "readyUnchecked":
      return {
        type: "info",
        title: t("vault.sync.status_ready"),
        description:
          display.reason === "checkFailed" && remoteStatus !== null
            ? remoteErrorText(remoteStatus, t)
            : t("vault.sync.remote_check_hint"),
        action: checkRemoteAction,
      };
  }
}
