import { Tooltip } from "antd";
import { WarningOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useVaultStore } from "./core/vaultStore";
import { deriveSyncDisplay, remoteErrorText } from "./core/syncDisplay";
import { VAULT_TONES } from "./core/vaultColors";
import { formatRelativeTime } from "@/utils/formatRelativeTime";
import styles from "./vault.module.css";

interface VaultStatusBarProps {
  entryCount: number;
  onOpenSettings?: () => void;
}

export default function VaultStatusBar({
  entryCount,
  onOpenSettings,
}: VaultStatusBarProps) {
  const { t } = useTranslation();
  const {
    hibpLeaks,
    syncReadyness,
    remoteStatus,
    syncLastAt,
    syncing,
    syncError,
    clearError,
    clearSyncError,
  } = useVaultStore();

  const leakCount = Object.values(hibpLeaks).filter((c) => c > 0).length;
  const display = deriveSyncDisplay({
    readyness: syncReadyness,
    remoteStatus,
    syncError: syncing ? null : syncError,
  });

  const dot = (color: string) => (
    <span className={styles.syncDot} style={{ backgroundColor: color }} />
  );

  const renderState = () => {
    switch (display.kind) {
      case "unknown":
        return null;
      case "failed":
        return (
          <Tooltip
            title={t("vault.sync.state_failed_tip", {
              message: display.message,
            })}
          >
            <button
              type="button"
              className={styles.syncFailed}
              onClick={() => {
                clearSyncError();
                clearError();
              }}
            >
              {dot(VAULT_TONES.syncFailed)}
              {t("vault.sync.state_failed")}
            </button>
          </Tooltip>
        );
      case "repoState":
        return (
          <Tooltip
            title={t("vault.sync.repo_state_blocked", {
              state: display.state,
            })}
          >
            <span className={styles.syncState}>
              {dot(VAULT_TONES.leak)}
              {t("vault.sync.state_conflict")}
            </span>
          </Tooltip>
        );
      case "noSync":
        return (
          <span className={styles.syncState}>
            {dot(VAULT_TONES.syncOff)}
            {t("vault.sync.state_off")}
          </span>
        );
      case "noRemote":
        return (
          <Tooltip title={t("vault.sync.remote_not_bound_tip")}>
            <button
              type="button"
              className={styles.syncState}
              style={{
                border: "none",
                background: "none",
                padding: 0,
                font: "inherit",
                cursor: onOpenSettings ? "pointer" : "default",
              }}
              onClick={onOpenSettings}
            >
              {dot(VAULT_TONES.syncOff)}
              {t("vault.sync.state_no_remote")}
            </button>
          </Tooltip>
        );
      case "dirty":
        return (
          <span className={styles.syncState}>
            {dot(VAULT_TONES.syncDirty)}
            {t("vault.sync.state_dirty")}
          </span>
        );
      case "ahead":
        return (
          <span className={styles.syncState}>
            {dot(VAULT_TONES.syncDirty)}
            {t("vault.sync.state_ahead", { count: display.ahead })}
          </span>
        );
      case "behind":
        return (
          <span className={styles.syncState}>
            {dot(VAULT_TONES.syncDirty)}
            {t("vault.sync.state_behind", { count: display.behind })}
          </span>
        );
      case "diverged":
        return (
          <span className={styles.syncState}>
            {dot(VAULT_TONES.leak)}
            {t("vault.sync.state_diverged", {
              ahead: display.ahead,
              behind: display.behind,
            })}
          </span>
        );
      case "inSync": {
        const time = new Date(display.checkedAt).toLocaleString();
        const tip = display.fetched
          ? t("vault.sync.remote_checked_at", { time })
          : t("vault.sync.remote_checked_stale", { time });
        return (
          <Tooltip title={tip}>
            <span className={styles.syncState}>
              {dot(VAULT_TONES.syncReady)}
              {t("vault.sync.state_in_sync")}
            </span>
          </Tooltip>
        );
      }
      case "readyUnchecked": {
        const tip =
          display.reason === "checkFailed" && remoteStatus !== null
            ? remoteErrorText(remoteStatus, t)
            : t("vault.sync.remote_check_hint");
        return (
          <Tooltip title={tip}>
            <span className={styles.syncState}>
              {dot(VAULT_TONES.syncOff)}
              {t("vault.sync.state_ready")}
            </span>
          </Tooltip>
        );
      }
    }
  };

  return (
    <div className={styles.syncBar}>
      <div className={styles.syncBarLeft}>
        <span>
          {entryCount} {t("vault.entry.entries_count")}
        </span>
        {leakCount > 0 && (
          <span className={styles.leakWarning}>
            <WarningOutlined style={{ marginRight: 4 }} />
            {leakCount} {t("vault.entry.leaked")}
          </span>
        )}
      </div>
      <div className={styles.syncBarRight}>
        {renderState()}
        {syncLastAt !== null && (
          <Tooltip
            title={t("vault.sync.last_sync", {
              time: new Date(syncLastAt).toLocaleString(),
            })}
          >
            <span className={styles.syncLastAt}>
              {formatRelativeTime(syncLastAt)}
            </span>
          </Tooltip>
        )}
      </div>
    </div>
  );
}
