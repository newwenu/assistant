import { useEffect } from "react";
import { Space, Typography, App } from "antd";
import { useTranslation } from "react-i18next";
import { useVaultStore } from "../core/vaultStore";
import DangerZone, { type DangerZoneAction } from "./DangerZone";

export default function DangerZoneTab() {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const {
    syncReadyness,
    syncConfig,
    syncing,
    loadSyncConfig,
    loadSyncReadyness,
    syncForcePush,
    syncDiscard,
    gitRemoveRemote,
    loadEntries,
  } = useVaultStore();

  useEffect(() => {
    void loadSyncConfig();
    void loadSyncReadyness();
  }, [loadSyncConfig, loadSyncReadyness]);

  const isNoSync = syncReadyness === "noSync";
  const hasRemote = Boolean(syncConfig?.remoteUrl);
  const remoteName = syncConfig?.remoteName ?? "origin";

  const handleDiscard = async () => {
    await syncDiscard();
    const error = useVaultStore.getState().error;
    if (error) {
      message.error(error);
      return;
    }
    await loadEntries();
    message.success(t("vault.sync.discarded"));
  };

  const handleRemoveRemote = async () => {
    await gitRemoveRemote(remoteName);
    const error = useVaultStore.getState().error;
    if (error) {
      message.error(error);
      return;
    }
    message.success(t("vault.sync.removed_remote"));
  };

  const actions: DangerZoneAction[] = [
    {
      key: "discard",
      title: t("vault.danger_zone.discard.title"),
      description: t("vault.danger_zone.discard.desc"),
      buttonLabel: t("vault.sync.discard"),
      disabled: isNoSync,
      loading: syncing,
      onConfirm: handleDiscard,
    },
    {
      key: "force_push",
      title: t("vault.danger_zone.force_push.title"),
      description: t("vault.danger_zone.force_push.desc"),
      buttonLabel: t("vault.sync.force_push"),
      disabled: !hasRemote,
      loading: syncing,
      onConfirm: syncForcePush,
    },
    {
      key: "remove_remote",
      title: t("vault.danger_zone.remove_remote.title"),
      description: t("vault.danger_zone.remove_remote.desc"),
      buttonLabel: t("vault.sync.remove_remote"),
      disabled: !hasRemote,
      loading: syncing,
      onConfirm: handleRemoveRemote,
    },
  ];

  return (
    <Space orientation="vertical" style={{ width: "100%" }} size="middle">
      <Typography.Text type="secondary">
        {t("vault.danger_zone.tab_desc")}
      </Typography.Text>
      <DangerZone actions={actions} />
    </Space>
  );
}
