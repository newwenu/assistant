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
  Dropdown,
  App,
} from "antd";
import {
  SyncOutlined,
  CloudUploadOutlined,
  CloudDownloadOutlined,
  LinkOutlined,
  ExclamationCircleOutlined,
  DisconnectOutlined,
  MoreOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { translateKey } from "@/i18n";
import { formatRelativeTime } from "@/utils/formatRelativeTime";
import { useSettingsStore } from "@/stores/settingsStore";
import { dispatchConfPatch } from "@/settings/core/settingsDispatcher";
import { useVaultStore } from "../core/vaultStore";
import type {
  SyncConfig,
  SyncResult,
  VaultErrorCode,
} from "../core/vault.types";
import DangerZoneConfirm from "./DangerZoneConfirm";

const SYNC_ERROR_KEY_MAP: Partial<Record<VaultErrorCode, string>> = {
  repoNotInitialized: "vault.sync.err_not_init",
  noRemoteConfigured: "vault.sync.err_no_remote",
};

function syncErrorText(code: VaultErrorCode | null, message: string): string {
  const i18nKey = code ? SYNC_ERROR_KEY_MAP[code] : undefined;
  return i18nKey
    ? translateKey(i18nKey)
    : translateKey("vault.sync.result_error", { message });
}

function getSyncResultInfo(
  result: SyncResult | null,
  t: TFunction,
): { type: "success" | "info" | "warning" | "error"; message: string } | null {
  if (!result) return null;
  if (result === "success")
    return {
      type: "success",
      message: t("vault.sync.result_success"),
    };
  if (result === "upToDate")
    return {
      type: "info",
      message: t("vault.sync.result_up_to_date"),
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

export default function VaultSyncSettings({
  onGoRecovery,
}: {
  onGoRecovery?: () => void;
} = {}) {
  const { t } = useTranslation();
  const { message, modal } = App.useApp();
  const {
    syncReadyness,
    syncConfig,
    syncing,
    syncResult,
    syncLastAt,
    pendingRemovals,
    error,
    errorCode,
    entries,
    syncNow,
    syncPull,
    syncPush,
    syncForcePush,
    loadSyncConfig,
    saveSyncConfig,
    loadSyncReadyness,
    loadPendingRemovals,
    loadSyncLastTime,
    confirmRemovals,
    loadEntries,
    gitInit,
    gitClone,
    gitRemoveRemote,
    gitTestConnection,
    syncAbort,
    syncDiscard,
  } = useVaultStore();

  const vaultConf = useSettingsStore((s) => s.vaultConf);
  const sshHostsVisible = !navigator.platform.toLowerCase().includes("win");
  const [remoteUrl, setRemoteUrl] = useState("");
  const [sshHosts, setSshHosts] = useState("");
  const [forcePushOpen, setForcePushOpen] = useState(false);
  const [testing, setTesting] = useState(false);

  useEffect(() => {
    void loadSyncConfig();
    void loadSyncReadyness();
    void loadPendingRemovals();
    void loadSyncLastTime();
    void loadEntries();
  }, [
    loadSyncConfig,
    loadSyncReadyness,
    loadPendingRemovals,
    loadSyncLastTime,
    loadEntries,
  ]);

  useEffect(() => {
    if (syncConfig) {
      setRemoteUrl(syncConfig.remoteUrl ?? "");
    }
  }, [syncConfig]);

  useEffect(() => {
    if (vaultConf) {
      setSshHosts((vaultConf.vaultSyncSshHosts ?? []).join(", "));
    }
  }, [vaultConf]);

  const isReady = syncReadyness === "ready";
  const isNoSync = syncReadyness === "noSync";
  const repoState =
    typeof syncReadyness === "object" && syncReadyness !== null
      ? syncReadyness.repoState
      : null;
  const hasRemote = !isNoSync && Boolean(syncConfig?.remoteUrl);
  const cloneBlocked = entries.length > 0;

  const buildConfig = (): SyncConfig => ({
    remoteUrl: remoteUrl.trim() || null,
    remoteName: "origin",
    sshKeyPath: null,
  });

  const handleInit = async () => {
    await gitInit();
    if (remoteUrl.trim()) {
      await saveSyncConfig(buildConfig());
    }
    await loadSyncConfig();
    await loadSyncReadyness();
  };

  const handleClone = async () => {
    const url = remoteUrl.trim();
    if (!url || cloneBlocked) return;
    await gitClone(url);
    await loadSyncConfig();
  };

  const handleSaveConfig = async () => {
    await saveSyncConfig(buildConfig());
    const hosts = sshHosts
      .split(",")
      .map((host) => host.trim().toLowerCase())
      .filter((host) => host.length > 0);
    await dispatchConfPatch("vault", { vaultSyncSshHosts: hosts });
  };

  const handleTestConnection = async () => {
    setTesting(true);
    const count = await gitTestConnection(remoteUrl.trim() || undefined);
    setTesting(false);
    if (count !== null) {
      message.success(t("vault.sync.test_ok", { count }));
    }
  };

  const handleRemoveRemote = () => {
    modal.confirm({
      title: t("vault.sync.remove_remote_confirm_title"),
      content: t("vault.sync.remove_remote_confirm_desc"),
      okType: "danger",
      onOk: async () => {
        await gitRemoveRemote(syncConfig?.remoteName ?? "origin");
        setRemoteUrl("");
        message.success(t("vault.sync.removed_remote"));
      },
    });
  };

  const handleAbort = () => {
    modal.confirm({
      title: t("vault.sync.abort_confirm_title"),
      content: t("vault.sync.abort_confirm_desc"),
      onOk: async () => {
        await syncAbort();
        message.success(t("vault.sync.aborted"));
      },
    });
  };

  const handleDiscard = () => {
    modal.confirm({
      title: t("vault.sync.discard_confirm_title"),
      content: t("vault.sync.discard_confirm_desc"),
      okType: "danger",
      onOk: async () => {
        await syncDiscard();
        message.success(t("vault.sync.discarded"));
      },
    });
  };

  const handleForcePush = useCallback(() => {
    setForcePushOpen(false);
    void syncForcePush();
  }, [syncForcePush]);

  const handleGoRecovery = () => {
    message.info(t("vault.sync.unreadable_export_hint"));
    onGoRecovery?.();
  };

  const handleOverwriteRemote = () => {
    setForcePushOpen(true);
  };

  const handleReviewPendingRemovals = () => {
    if (pendingRemovals === null) return;
    modal.confirm({
      title:
        pendingRemovals.entries.length >= pendingRemovals.totalLocal
          ? t("vault.sync.pending_removals_all_title", {
              count: pendingRemovals.entries.length,
            })
          : t("vault.sync.pending_removals_title", {
              count: pendingRemovals.entries.length,
            }),
      content: t("vault.sync.pending_removals_desc"),
      okText: t("vault.sync.pending_removals_confirm"),
      cancelText: t("vault.sync.pending_removals_cancel"),
      okButtonProps: { danger: true },
      onOk: () => {
        void confirmRemovals(pendingRemovals.entries.map((e) => e.id));
      },
    });
  };

  const readynessText = () => {
    if (isNoSync) return t("vault.sync.status_no_sync");
    if (syncReadyness === "dirty") return t("vault.sync.status_dirty");
    if (isReady) return t("vault.sync.status_ready");
    if (repoState) {
      return t("vault.sync.repo_state_blocked", { state: repoState });
    }
    return t("vault.sync.status_unknown");
  };

  const resultInfo = getSyncResultInfo(syncResult, t);

  const unreadable =
    syncResult !== null &&
    typeof syncResult === "object" &&
    "unreadableEntries" in syncResult
      ? syncResult.unreadableEntries
      : null;
  const keyMismatchError = errorCode === "keyMismatch";
  const unreadableView = unreadable
    ? unreadable
    : keyMismatchError
      ? { ids: [] as string[], reason: "keyMismatch" as const }
      : null;

  return (
    <Spin spinning={syncing}>
      <Space orientation="vertical" style={{ width: "100%" }} size="middle">
        <Alert
          type={isReady ? "success" : isNoSync ? "info" : "warning"}
          title={readynessText()}
          showIcon
        />

        {pendingRemovals !== null && (
          <Alert
            type="warning"
            showIcon
            title={t("vault.sync.pending_removals_banner", {
              count: pendingRemovals.entries.length,
            })}
            description={t("vault.sync.pending_removals_hint")}
            action={
              <Button size="small" onClick={handleReviewPendingRemovals}>
                {t("vault.sync.pending_removals_open")}
              </Button>
            }
          />
        )}

        {error && !keyMismatchError && (
          <Alert
            type="error"
            title={syncErrorText(errorCode, error)}
            showIcon
            closable={{
              onClose: () => useVaultStore.getState().clearError(),
            }}
          />
        )}

        {unreadableView && (
          <Alert
            type="error"
            showIcon
            title={
              unreadableView.ids.length > 0
                ? t("vault.sync.unreadable_title", {
                    count: unreadableView.ids.length,
                  })
                : t("vault.sync.unreadable_title_all")
            }
            description={
              <Space orientation="vertical" size={4} style={{ width: "100%" }}>
                <Typography.Text>
                  {unreadableView.reason === "keyMismatch"
                    ? t("vault.sync.unreadable_reason_key_mismatch")
                    : t("vault.sync.unreadable_reason_corrupt")}
                </Typography.Text>
                {unreadableView.ids.length > 0 && (
                  <Typography.Text
                    type="secondary"
                    style={{ wordBreak: "break-all" }}
                  >
                    {unreadableView.ids.join(", ")}
                  </Typography.Text>
                )}
                <Space>
                  <Button size="small" onClick={handleGoRecovery}>
                    {t("vault.sync.unreadable_export")}
                  </Button>
                  <Button
                    size="small"
                    danger
                    disabled={!hasRemote}
                    onClick={handleOverwriteRemote}
                  >
                    {t("vault.sync.unreadable_overwrite")}
                  </Button>
                </Space>
              </Space>
            }
          />
        )}

        {resultInfo && (
          <Alert type={resultInfo.type} title={resultInfo.message} showIcon />
        )}

        {!isNoSync && (
          <Typography.Text type="secondary">
            {syncLastAt === null
              ? t("vault.sync.last_sync_never")
              : t("vault.sync.last_sync", {
                  time: formatRelativeTime(syncLastAt),
                })}
          </Typography.Text>
        )}

        <Form layout="vertical">
          <Form.Item label={t("vault.sync.remote_url")}>
            <Space.Compact style={{ width: "100%" }}>
              <Input
                value={remoteUrl}
                onChange={(e) => setRemoteUrl(e.target.value)}
                placeholder="git@github.com:user/vault.git"
              />
              <Button
                icon={<LinkOutlined />}
                loading={testing}
                disabled={!remoteUrl.trim()}
                onClick={() => void handleTestConnection()}
              >
                {t("vault.sync.test_connection")}
              </Button>
            </Space.Compact>
          </Form.Item>

          {sshHostsVisible && (
            <Form.Item
              label={t("vault.sync.ssh_hosts")}
              extra={t("vault.sync.ssh_hosts_hint")}
            >
              <Input
                value={sshHosts}
                onChange={(e) => setSshHosts(e.target.value)}
                placeholder="github.com, gitlab.com"
              />
            </Form.Item>
          )}

          {!isNoSync && (
            <Form.Item>
              <Space>
                <Button type="primary" onClick={() => void handleSaveConfig()}>
                  {t("common.actions.save")}
                </Button>
                {hasRemote && (
                  <Button
                    danger
                    icon={<DisconnectOutlined />}
                    onClick={handleRemoveRemote}
                  >
                    {t("vault.sync.remove_remote")}
                  </Button>
                )}
              </Space>
            </Form.Item>
          )}
        </Form>

        {isNoSync && (
          <Space orientation="vertical" style={{ width: "100%" }} size="small">
            <Typography.Text type="secondary">
              {t("vault.sync.first_setup_hint")}
            </Typography.Text>
            {cloneBlocked && (
              <Alert
                type="warning"
                showIcon
                title={t("vault.sync.clone_blocked_local_entries", {
                  count: entries.length,
                })}
              />
            )}
            <Space wrap>
              <Button
                type="primary"
                icon={<LinkOutlined />}
                onClick={() => void handleInit()}
              >
                {t("vault.sync.init_local")}
              </Button>
              <Button
                icon={<CloudDownloadOutlined />}
                disabled={!remoteUrl.trim() || cloneBlocked}
                onClick={() => void handleClone()}
              >
                {t("vault.sync.clone")}
              </Button>
            </Space>
          </Space>
        )}

        {!isNoSync && (
          <>
            <Divider />

            {repoState && (
              <Alert
                type="warning"
                showIcon
                title={t("vault.sync.repo_state_blocked", {
                  state: repoState,
                })}
                action={
                  <Button size="small" onClick={handleAbort}>
                    {t("vault.sync.abort")}
                  </Button>
                }
              />
            )}

            <Space>
              <Button
                type="primary"
                icon={<SyncOutlined />}
                onClick={() => void syncNow()}
                loading={syncing}
              >
                {t("vault.sync.now")}
              </Button>
              <Dropdown
                disabled={syncing}
                menu={{
                  items: [
                    {
                      key: "pull",
                      icon: <CloudDownloadOutlined />,
                      label: t("vault.sync.pull"),
                      onClick: () => void syncPull(),
                    },
                    {
                      key: "push",
                      icon: <CloudUploadOutlined />,
                      label: t("vault.sync.push"),
                      disabled: pendingRemovals !== null,
                      onClick: () => void syncPush(),
                    },
                  ],
                }}
              >
                <Button icon={<MoreOutlined />}>
                  {t("common.actions.more")}
                </Button>
              </Dropdown>
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
                  {t("vault.danger_zone.title")}
                </Typography.Text>
                <Typography.Text type="secondary">
                  {t("vault.danger_zone.desc")}
                </Typography.Text>
                <Space wrap>
                  <Button
                    danger
                    onClick={() => setForcePushOpen(true)}
                    loading={syncing}
                  >
                    {t("vault.sync.force_push")}
                  </Button>
                  <Button danger onClick={handleDiscard} loading={syncing}>
                    {t("vault.sync.discard")}
                  </Button>
                </Space>
              </Space>
            </div>
          </>
        )}
      </Space>

      <DangerZoneConfirm
        open={forcePushOpen}
        title={t("vault.sync.force_push_confirm_title")}
        description={t("vault.sync.force_push_confirm_desc")}
        confirmLabel={t("vault.sync.force_push")}
        onConfirm={handleForcePush}
        onCancel={() => setForcePushOpen(false)}
        loading={syncing}
      />
    </Spin>
  );
}
