import { useEffect, useState } from "react";
import {
  Form,
  Input,
  InputNumber,
  Button,
  Space,
  Alert,
  Spin,
  Divider,
} from "antd";
import {
  SyncOutlined,
  CloudUploadOutlined,
  CloudDownloadOutlined,
  LinkOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useVaultStore } from "./vaultStore";
import type { SyncConfig } from "./vault.service";

export default function VaultSyncSettings() {
  const { t } = useTranslation();
  const {
    syncReadyness,
    syncConfig,
    syncing,
    syncNow,
    syncPull,
    syncPush,
    syncForcePush,
    loadSyncConfig,
    saveSyncConfig,
    loadSyncReadyness,
    gitInit,
    gitAddRemote,
  } = useVaultStore();

  const [remoteUrl, setRemoteUrl] = useState("");
  const [autoSyncInterval, setAutoSyncInterval] = useState(300);

  useEffect(() => {
    void loadSyncConfig();
    void loadSyncReadyness();
  }, [loadSyncConfig, loadSyncReadyness]);

  useEffect(() => {
    if (syncConfig) {
      setRemoteUrl(syncConfig.remoteUrl ?? "");
      setAutoSyncInterval(syncConfig.autoSyncIntervalSecs);
    }
  }, [syncConfig]);

  const isReady = syncReadyness === "ready";
  const isNoSync = syncReadyness === "noSync";
  const isDirty = syncReadyness === "dirty";

  const handleInit = async () => {
    await gitInit();
    if (remoteUrl) {
      await gitAddRemote("origin", remoteUrl);
    }
  };

  const handleSaveConfig = async () => {
    const config: SyncConfig = {
      remoteUrl: remoteUrl || null,
      remoteName: "origin",
      branch: "main",
      autoSyncIntervalSecs: autoSyncInterval,
      sshKeyPath: null,
    };
    await saveSyncConfig(config);
  };

  const readynessText = () => {
    if (isNoSync) return t("tools.password_vault.sync_status_no_sync");
    if (isDirty) return t("tools.password_vault.sync_status_dirty");
    if (isReady) return t("tools.password_vault.sync_status_ready");
    return t("tools.password_vault.sync_status_unknown");
  };

  return (
    <Spin spinning={syncing}>
      <Space direction="vertical" style={{ width: "100%" }} size="middle">
        <Alert
          type={isReady ? "success" : isNoSync ? "info" : "warning"}
          message={readynessText()}
          showIcon
        />

        {isNoSync && (
          <Button
            type="primary"
            icon={<LinkOutlined />}
            onClick={() => void handleInit()}
          >
            {t("tools.password_vault.sync_init")}
          </Button>
        )}

        {!isNoSync && (
          <>
            <Form layout="vertical">
              <Form.Item label={t("tools.password_vault.sync_remote_url")}>
                <Input
                  value={remoteUrl}
                  onChange={(e) => setRemoteUrl(e.target.value)}
                  placeholder="git@github.com:user/vault.git"
                />
              </Form.Item>

              <Form.Item label={t("tools.password_vault.sync_auto_interval")}>
                <InputNumber
                  min={0}
                  max={3600}
                  value={autoSyncInterval}
                  onChange={(v) => setAutoSyncInterval(v ?? 300)}
                  addonAfter={t("tools.password_vault.seconds")}
                />
              </Form.Item>

              <Form.Item>
                <Button type="primary" onClick={() => void handleSaveConfig()}>
                  {t("common.actions.save")}
                </Button>
              </Form.Item>
            </Form>

            <Divider />

            <Space>
              <Button
                icon={<CloudDownloadOutlined />}
                onClick={() => void syncPull()}
                loading={syncing}
              >
                {t("tools.password_vault.sync_pull")}
              </Button>
              <Button
                icon={<CloudUploadOutlined />}
                onClick={() => void syncPush()}
                loading={syncing}
              >
                {t("tools.password_vault.sync_push")}
              </Button>
              <Button
                type="primary"
                icon={<SyncOutlined />}
                onClick={() => void syncNow()}
                loading={syncing}
              >
                {t("tools.password_vault.sync_now")}
              </Button>
            </Space>

            <Divider />

            <Button
              danger
              onClick={() => void syncForcePush()}
              loading={syncing}
            >
              {t("tools.password_vault.sync_force_push")}
            </Button>
          </>
        )}
      </Space>
    </Spin>
  );
}
