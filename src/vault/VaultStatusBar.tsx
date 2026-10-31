import { Tooltip } from "antd";
import { WarningOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useVaultStore } from "./core/vaultStore";
import type { Readyness, RepositoryState } from "./core/vault.types";
import { VAULT_TONES } from "./core/vaultColors";
import { formatRelativeTime } from "@/utils/formatRelativeTime";
import styles from "./vault.module.css";

type SyncStatusKey =
  | "vault.sync.state_conflict"
  | "vault.sync.state_dirty"
  | "vault.sync.state_ready"
  | "vault.sync.state_off";

function getSyncStatus(
  readyness: Readyness | null,
  repoState: RepositoryState | null,
): { label: SyncStatusKey; color: string } | null {
  if (readyness === null) return null;
  if (repoState !== null) {
    return { label: "vault.sync.state_conflict", color: VAULT_TONES.leak };
  }
  if (readyness === "dirty") {
    return { label: "vault.sync.state_dirty", color: VAULT_TONES.syncDirty };
  }
  if (readyness === "ready") {
    return { label: "vault.sync.state_ready", color: VAULT_TONES.syncReady };
  }
  return { label: "vault.sync.state_off", color: VAULT_TONES.syncOff };
}

interface VaultStatusBarProps {
  entryCount: number;
}

export default function VaultStatusBar({ entryCount }: VaultStatusBarProps) {
  const { t } = useTranslation();
  const {
    hibpLeaks,
    syncReadyness,
    syncLastAt,
    syncing,
    syncError,
    clearError,
    clearSyncError,
  } = useVaultStore();

  const leakCount = Object.values(hibpLeaks).filter((c) => c > 0).length;
  const repoState =
    syncReadyness !== null && typeof syncReadyness === "object"
      ? syncReadyness.repoState
      : null;
  const syncStatus = getSyncStatus(syncReadyness, repoState);

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
        {syncError !== null && !syncing ? (
          <Tooltip
            title={t("vault.sync.state_failed_tip", { message: syncError })}
          >
            <button
              type="button"
              className={styles.syncFailed}
              onClick={() => {
                clearSyncError();
                clearError();
              }}
            >
              <span
                className={styles.syncDot}
                style={{ backgroundColor: VAULT_TONES.syncFailed }}
              />
              {t("vault.sync.state_failed")}
            </button>
          </Tooltip>
        ) : (
          syncStatus !== null && (
            <span className={styles.syncState}>
              <span
                className={styles.syncDot}
                style={{ backgroundColor: syncStatus.color }}
              />
              {t(syncStatus.label)}
            </span>
          )
        )}
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
