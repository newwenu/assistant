import React from "react";
import { Button, Empty, Spin } from "antd";
import { useTranslation } from "react-i18next";
import { SettingsPageShell } from "../../shared/layout/SettingsPageShell";
import { AttachmentSection } from "./AttachmentSection";
import { EntrySection } from "./EntrySection";
import { ResidualSection } from "./ResidualSection";
import { SnapshotToolbar } from "./SnapshotToolbar";
import { useStorageSnapshot } from "./useStorageSnapshot";

export const StorageContent: React.FC = () => {
  const { t } = useTranslation();
  const snapshot = useStorageSnapshot();

  const { snapshot: breakdown, phase, error } = snapshot;

  if (breakdown === null) {
    return (
      <SettingsPageShell wide>
        {phase === "initial" && error === null ? (
          <div className="storage-loading">
            <Spin />
          </div>
        ) : (
          <Empty
            image={Empty.PRESENTED_IMAGE_SIMPLE}
            description={
              <div className="storage-load-error">
                <span>{t("settings.storage.common.load_failed")}</span>
                {error !== null && (
                  <span className="storage-load-error-detail">{error}</span>
                )}
              </div>
            }
          >
            <Button size="small" onClick={snapshot.retry}>
              {t("settings.storage.common.retry")}
            </Button>
          </Empty>
        )}
      </SettingsPageShell>
    );
  }

  return (
    <SettingsPageShell wide>
      <SnapshotToolbar
        sampledAt={breakdown.sampledAt}
        refreshing={phase === "refreshing"}
        error={error}
        onRefresh={snapshot.refresh}
      />

      <AttachmentSection
        breakdown={breakdown}
        onDelete={snapshot.deleteAttachments}
      />

      <EntrySection
        entries={breakdown.entries}
        onReclaimed={snapshot.refresh}
      />

      <ResidualSection
        inventory={breakdown.inventory}
        onScan={snapshot.scanInventory}
      />
    </SettingsPageShell>
  );
};
