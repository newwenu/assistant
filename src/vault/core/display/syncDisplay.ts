import type { TFunction } from "i18next";
import type { Readyness, RemoteStatus, RepositoryState } from "../vault.types";

const URL_USERINFO_RE = /\b[a-z][a-z0-9+.-]*:\/\/[^/\s"']*@/gi;

export function redactUrlUserInfo(text: string): string {
  return text.replace(URL_USERINFO_RE, (match: string) => {
    const schemeEnd = match.indexOf("://") + 3;
    return match.slice(0, schemeEnd);
  });
}

export function remoteErrorText(status: RemoteStatus, t: TFunction): string {
  const detail = status.errorDetail
    ? redactUrlUserInfo(status.errorDetail)
    : null;
  switch (status.errorKind) {
    case "noRemote":
      return t("vault.sync.remote_check_no_remote");
    case "noUpstream":
      return t("vault.sync.remote_check_no_upstream");
    case "fetchFailed":
    case "compareFailed":
      return detail
        ? t("vault.sync.remote_check_failed", { message: detail })
        : t("vault.sync.remote_check_failed_generic");
    default:
      return t("vault.sync.remote_check_hint");
  }
}

export type SyncDisplay =
  | { kind: "unknown" }
  | { kind: "failed"; message: string }
  | { kind: "repoState"; state: RepositoryState }
  | { kind: "noSync" }
  | { kind: "noRemote" }
  | { kind: "dirty" }
  | { kind: "ahead"; ahead: number }
  | { kind: "behind"; behind: number }
  | { kind: "diverged"; ahead: number; behind: number }
  | { kind: "inSync"; checkedAt: number; fetched: boolean }
  | { kind: "readyUnchecked"; reason: "notChecked" | "checkFailed" };

export function deriveSyncDisplay(input: {
  readyness: Readyness | null;
  remoteStatus: RemoteStatus | null;
  syncError: string | null;
}): SyncDisplay {
  const { readyness, remoteStatus, syncError } = input;

  if (readyness !== null && typeof readyness === "object") {
    return { kind: "repoState", state: readyness.repoState };
  }
  if (syncError !== null) {
    return { kind: "failed", message: redactUrlUserInfo(syncError) };
  }
  if (readyness === null) {
    return { kind: "unknown" };
  }
  if (readyness === "noSync") {
    return { kind: "noSync" };
  }
  if (readyness === "noRemote") {
    return { kind: "noRemote" };
  }
  if (readyness === "dirty") {
    return { kind: "dirty" };
  }

  const ahead = remoteStatus?.ahead ?? null;
  const behind = remoteStatus?.behind ?? null;
  const countsUnreliable =
    remoteStatus?.errorKind === "fetchFailed" ||
    remoteStatus?.errorKind === "compareFailed";

  if (ahead !== null && behind !== null) {
    if (ahead === 0 && behind === 0) {
      if (countsUnreliable) {
        return { kind: "readyUnchecked", reason: "checkFailed" };
      }
      return {
        kind: "inSync",
        checkedAt: remoteStatus?.checkedAt ?? 0,
        fetched: remoteStatus?.fetched ?? false,
      };
    }
    if (ahead > 0 && behind === 0) {
      return { kind: "ahead", ahead };
    }
    if (ahead === 0 && behind > 0) {
      return { kind: "behind", behind };
    }
    return { kind: "diverged", ahead, behind };
  }

  return {
    kind: "readyUnchecked",
    reason: remoteStatus === null ? "notChecked" : "checkFailed",
  };
}
