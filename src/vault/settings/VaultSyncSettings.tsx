import { useEffect, useState, useCallback } from "react";
import {
  Form,
  Input,
  Button,
  Space,
  Alert,
  Spin,
  Divider,
  Typography,
} from "antd";
import {
  SyncOutlined,
  CloudUploadOutlined,
  CloudDownloadOutlined,
  LinkOutlined,
  ExclamationCircleOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { translateKey } from "@/i18n";
import { useVaultStore } from "../core/vaultStore";
import type { SyncConfig, SyncResult } from "../core/vault.service";
import DangerZoneConfirm from "./DangerZoneConfirm";

const RUST_ERROR_KEY_MAP: Record<string, string> = {
  "Git repository not initialized": "tools.password_vault.sync_err_not_init",
  "No remote configured": "tools.password_vault.sync_err_no_remote",
};

function getSyncResultInfo(
  result: SyncResult | null,
  t: TFunction,
): { type: "success" | "info" | "warning" | "error"; message: string } | null {
  if (!result) return null;
  if (result === "success")
    return { type: "success", message: t("tools.password_vault.sync_result_success") as string };
  if (result === "upToDate")
    return { type: "info", message: t("tools.password_vault.sync_result_up_to_date") as string };
  if ("conflict" in result)
    return {
      type: "warning",
      message: t("tools.password_vault.sync_result_conflict", {
        count: result.conflict.length,
      }) as string,
    };
  if ("error" in result) {
    const i18nKey = RUST_ERROR_KEY_MAP[result.error];
    const message = i18nKey
      ? translateKey(i18nKey)
      : translateKey("tools.password_vault.sync_result_error", { message: result.error });
    return { type: "error", message };
  }
  return null;
}

export default function VaultSyncSettings() {
  const { t } = useTranslation();
  const {
    syncReadyness,
    syncConfig,
    syncing,
    syncResult,
    error,
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
  const [forcePushOpen, setForcePushOpen] = useState(false);

  useEffect(() => {
    void loadSyncConfig();
    void loadSyncReadyness();
  }, [loadSyncConfig, loadSyncReadyness]);

  useEffect(() => {
    if (syncConfig) {
      setRemoteUrl(syncConfig.remoteUrl ?? "");
    }
  }, [syncConfig]);

  const isReady = syncReadyness === "ready";
  const isNoSync = syncReadyness === "noSync";

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
      sshKeyPath: null,
    };
    await saveSyncConfig(config);
  };

  const handleForcePush = useCallback(() => {
    setForcePushOpen(false);
    void syncForcePush();
  }, [syncForcePush]);

  const readynessText = () => {
    if (isNoSync) return t("tools.password_vault.sync_status_no_sync");
    if (syncReadyness === "dirty")
      return t("tools.password_vault.sync_status_dirty");
    if (isReady) return t("tools.password_vault.sync_status_ready");
    return t("tools.password_vault.sync_status_unknown");
  };

  const resultInfo = getSyncResultInfo(syncResult, t);

  return (
    <Spin spinning={syncing}>
      <Space orientation="vertical" style={{ width: "100%" }} size="middle">
        <Alert
          type={isReady ? "success" : isNoSync ? "info" : "warning"}
          title={readynessText()}
          showIcon
        />

        {error && (
          <Alert
            type="error"
            title={error}
            showIcon
            closable
            onClose={() => useVaultStore.getState().clearError()}
          />
        )}

        {resultInfo && (
          <Alert
            type={resultInfo.type}
            title={resultInfo.message}
            showIcon
          />
        )}

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

            <div
              style={{
                border: "1px solid var(--ant-color-error-border)",
                borderRadius: 8,
                padding: "16px 24px",
                background: "var(--ant-color-error-bg)",
              }}
            >
              <Space
                orientation="vertical"
                style={{ width: "100%" }}
                size="small"
              >
                <Typography.Text strong type="danger">
                  <ExclamationCircleOutlined style={{ marginRight: 6 }} />
                  {t("tools.password_vault.danger_zone_title")}
                </Typography.Text>
                <Typography.Text type="secondary">
                  {t("tools.password_vault.danger_zone_desc")}
                </Typography.Text>
                <Button
                  danger
                  onClick={() => setForcePushOpen(true)}
                  loading={syncing}
                >
                  {t("tools.password_vault.sync_force_push")}
                </Button>
              </Space>
            </div>
          </>
        )}
      </Space>

      <DangerZoneConfirm
        open={forcePushOpen}
        title={t("tools.password_vault.sync_force_push_confirm_title")}
        description={t("tools.password_vault.sync_force_push_confirm_desc")}
        confirmLabel={t("tools.password_vault.sync_force_push")}
        onConfirm={handleForcePush}
        onCancel={() => setForcePushOpen(false)}
        loading={syncing}
      />
    </Spin>
  );
}