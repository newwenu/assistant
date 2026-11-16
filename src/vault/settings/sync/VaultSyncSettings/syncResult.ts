import type { TFunction } from "i18next";
import { translateKey } from "@/i18n";

import { redactUrlUserInfo } from "../../../core/display/syncDisplay";
import type {
  SyncResult,
  UnreadableEntries,
  VaultErrorCode,
} from "../../../core/vault.types";

const SYNC_ERROR_KEY_MAP: Partial<Record<VaultErrorCode, string>> = {
  repoNotInitialized: "vault.sync.err_not_init",
  noRemoteConfigured: "vault.sync.err_no_remote",
};

export function syncErrorText(
  code: VaultErrorCode | null,
  message: string,
): string {
  const i18nKey = code ? SYNC_ERROR_KEY_MAP[code] : undefined;
  return i18nKey
    ? translateKey(i18nKey)
    : translateKey("vault.sync.result_error", {
        message: redactUrlUserInfo(message),
      });
}

export interface SyncResultInfo {
  type: "success" | "info" | "warning" | "error";
  message: string;
}

export function getSyncResultInfo(
  result: SyncResult | null,
  t: TFunction,
): SyncResultInfo | null {
  if (!result) return null;
  if (result === "success")
    return {
      type: "success",
      message: t("vault.sync.result_success"),
    };
  if (result === "noRemote")
    return {
      type: "info",
      message: t("vault.sync.result_no_remote"),
    };
  if ("conflict" in result)
    return {
      type: "warning",
      message: t("vault.sync.result_conflict", {
        count: result.conflict.length,
      }),
    };
  if ("pendingRemovals" in result)
    return {
      type: "warning",
      message: t("vault.sync.pending_removals_banner", {
        count: result.pendingRemovals.entries.length,
      }),
    };
  if ("unreadableEntries" in result)
    return {
      type: "warning",
      message: t("vault.sync.result_unreadable", {
        count: result.unreadableEntries.ids.length,
      }),
    };
  return null;
}

/**
 * 不可读条目视图：优先取同步结果里的明细；若当前错误码是 `keyMismatch`
 * （结果里没有明细），退化为「全部条目不可读」的空 ids + keyMismatch 原因。
 */
export function deriveUnreadableView(
  syncResult: SyncResult | null,
  errorCode: VaultErrorCode | null,
): UnreadableEntries | null {
  const unreadable =
    syncResult !== null &&
    typeof syncResult === "object" &&
    "unreadableEntries" in syncResult
      ? syncResult.unreadableEntries
      : null;
  if (unreadable) return unreadable;
  return errorCode === "keyMismatch"
    ? { ids: [], reason: "keyMismatch" }
    : null;
}
