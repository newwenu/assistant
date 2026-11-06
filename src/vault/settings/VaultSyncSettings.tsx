import { useEffect, useState, useRef } from "react";
import type { ReactNode } from "react";
import type { InputRef } from "antd";
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
  Tooltip,
  App,
} from "antd";
import {
  SyncOutlined,
  CloudSyncOutlined,
  CloudUploadOutlined,
  CloudDownloadOutlined,
  LinkOutlined,
  ExclamationCircleOutlined,
  MoreOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { translateKey } from "@/i18n";
import { formatRelativeTime } from "@/utils/formatRelativeTime";
import { useConfGroup } from "@/settings/core/useConfGroup";
import { dispatchConfPatch } from "@/settings/core/settingsDispatcher";
import { useVaultShallow, useVaultStore } from "../core/vaultStore";
import { deriveSyncDisplay, remoteErrorText } from "../core/syncDisplay";
import type {
  SyncConfig,
  SyncResult,
  VaultErrorCode,
} from "../core/vault.types";

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

export default function VaultSyncSettings({
  onGoRecovery,
  onGoDangerZone,
}: {
  onGoRecovery?: () => void;
  onGoDangerZone?: () => void;
} = {}) {
  const { t } = useTranslation();
  const { message, modal } = App.useApp();
  const {
    syncReadyness,
    remoteStatus,
    remoteChecking,
    syncConfig,
    syncing,
    syncResult,
    syncError,
    syncLastAt,
    pendingRemovals,
    error,
    errorCode,
    entries,
    syncNow,
    syncPull,
    syncPush,
    loadSyncConfig,
    saveSyncConfig,
    loadSyncReadyness,
    loadRemoteStatus,
    loadPendingRemovals,
    loadSyncLastTime,
    confirmRemovals,
    loadEntries,
    gitInit,
    gitClone,
    gitTestConnection,
    syncAbort,
  } = useVaultShallow((s) => ({
    syncReadyness: s.syncReadyness,
    remoteStatus: s.remoteStatus,
    remoteChecking: s.remoteChecking,
    syncConfig: s.syncConfig,
    syncing: s.syncing,
    syncResult: s.syncResult,
    syncError: s.syncError,
    syncLastAt: s.syncLastAt,
    pendingRemovals: s.pendingRemovals,
    error: s.error,
    errorCode: s.errorCode,
    entries: s.entries,
    syncNow: s.syncNow,
    syncPull: s.syncPull,
    syncPush: s.syncPush,
    loadSyncConfig: s.loadSyncConfig,
    saveSyncConfig: s.saveSyncConfig,
    loadSyncReadyness: s.loadSyncReadyness,
    loadRemoteStatus: s.loadRemoteStatus,
    loadPendingRemovals: s.loadPendingRemovals,
    loadSyncLastTime: s.loadSyncLastTime,
    confirmRemovals: s.confirmRemovals,
    loadEntries: s.loadEntries,
    gitInit: s.gitInit,
    gitClone: s.gitClone,
    gitTestConnection: s.gitTestConnection,
    syncAbort: s.syncAbort,
  }));

  const vaultConf = useConfGroup("vault");
  const sshHostsVisible = !navigator.platform.toLowerCase().includes("win");
  const [remoteUrl, setRemoteUrl] = useState("");
  const [sshHosts, setSshHosts] = useState("");
  const [testing, setTesting] = useState(false);
  const remoteUrlRef = useRef<InputRef>(null);

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

  const isNoSync = syncReadyness === "noSync";
  const isNoRemote = syncReadyness === "noRemote";
  const cloneBlocked = entries.length > 0;
  const display = deriveSyncDisplay({
    readyness: syncReadyness,
    remoteStatus,
    syncError,
  });
  const hasRemote = !isNoSync && !isNoRemote && Boolean(syncConfig?.remoteUrl);

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
    try {
      await saveSyncConfig(buildConfig());
      const hosts = sshHosts
        .split(",")
        .map((host) => host.trim().toLowerCase())
        .filter((host) => host.length > 0);
      await dispatchConfPatch("vault", { vaultSyncSshHosts: hosts });
    } catch (e) {
      const detail = e instanceof Error ? e.message : String(e);
      message.error(
        translateKey("common.actions.save_failed_desc", { error: detail }),
      );
    }
  };

  const handleTestConnection = async () => {
    setTesting(true);
    const count = await gitTestConnection(remoteUrl.trim() || undefined);
    setTesting(false);
    if (count !== null) {
      message.success(t("vault.sync.test_ok", { count }));
    }
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

  const handleGoRecovery = () => {
    message.info(t("vault.sync.unreadable_export_hint"));
    onGoRecovery?.();
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

  const checkRemoteAction: ReactNode = hasRemote ? (
    <Button
      size="small"
      icon={<CloudSyncOutlined />}
      loading={remoteChecking}
      onClick={() => void loadRemoteStatus(true)}
    >
      {remoteChecking
        ? t("vault.sync.remote_checking")
        : t("vault.sync.remote_check")}
    </Button>
  ) : undefined;

  const syncAlert = ((): {
    type: "success" | "info" | "warning" | "error";
    title: ReactNode;
    description?: ReactNode;
    action?: ReactNode;
  } => {
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
            <Button size="small" onClick={handleAbort}>
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
            <Button size="small" onClick={() => remoteUrlRef.current?.focus()}>
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
  })();

  return (
    <Spin spinning={syncing}>
      <Space orientation="vertical" style={{ width: "100%" }} size="middle">
        <Alert
          type={syncAlert.type}
          title={syncAlert.title}
          description={syncAlert.description}
          action={syncAlert.action}
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
              onClose: () => {
                useVaultStore.getState().clearError();
                useVaultStore.getState().clearSyncError();
              },
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
                  <Typography.Text
                    type="secondary"
                    style={{ fontSize: 12, alignSelf: "center" }}
                  >
                    {t("vault.danger_zone.unreadable_overwrite_hint")}
                  </Typography.Text>
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
                ref={remoteUrlRef}
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
              <Button type="primary" onClick={() => void handleSaveConfig()}>
                {t("common.actions.save")}
              </Button>
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
                      label: (
                        <Tooltip
                          title={t("vault.sync.push_hint_commits_first")}
                          placement="left"
                        >
                          <span>{t("vault.sync.push")}</span>
                        </Tooltip>
                      ),
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
                {t("vault.danger_zone.sync_hint")}
              </Typography.Text>
              {onGoDangerZone && (
                <Button size="small" danger onClick={onGoDangerZone}>
                  {t("vault.danger_zone.open")}
                </Button>
              )}
            </Space>
          </>
        )}
      </Space>
    </Spin>
  );
}
