import { useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import type { InputRef } from "antd";
import { Alert, Button, Space, Spin, Typography, App } from "antd";
import {
  CloudSyncOutlined,
  CloudDownloadOutlined,
  LinkOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { translateKey } from "@/i18n";
import { formatRelativeTime } from "@/utils/formatRelativeTime";
import { useConfGroup } from "@/settings/core/runtime/useConfGroup";
import { dispatchConfPatch } from "@/settings/core/runtime/settingsDispatcher";
import { useVaultShallow, useVaultStore } from "../../../core/vaultStore";
import {
  deriveSyncDisplay,
  redactUrlUserInfo,
} from "../../../core/display/syncDisplay";
import type { SyncConfig } from "../../../core/vault.types";

import { RemoteConfigForm } from "./RemoteConfigForm";
import { SyncActions } from "./SyncActions";
import { SyncNotices } from "./SyncNotices";
import { buildSyncAlert } from "./statusAlert";
import { deriveUnreadableView, getSyncResultInfo } from "./syncResult";

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
      const detail = redactUrlUserInfo(
        e instanceof Error ? e.message : String(e),
      );
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

  const handleDismissError = () => {
    useVaultStore.getState().clearError();
    useVaultStore.getState().clearSyncError();
  };

  const resultInfo = getSyncResultInfo(syncResult, t);
  const keyMismatchError = errorCode === "keyMismatch";
  const unreadableView = deriveUnreadableView(syncResult, errorCode);

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

  const syncAlert = buildSyncAlert({
    display,
    t,
    errorCode,
    remoteStatus,
    checkRemoteAction,
    onAbort: handleAbort,
    onFocusRemoteUrl: () => remoteUrlRef.current?.focus(),
  });

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

        <SyncNotices
          pendingRemovals={pendingRemovals}
          error={error}
          errorCode={errorCode}
          keyMismatchError={keyMismatchError}
          unreadableView={unreadableView}
          resultInfo={resultInfo}
          onReviewPendingRemovals={handleReviewPendingRemovals}
          onDismissError={handleDismissError}
          onGoRecovery={handleGoRecovery}
        />

        {!isNoSync && (
          <Typography.Text type="secondary">
            {syncLastAt === null
              ? t("vault.sync.last_sync_never")
              : t("vault.sync.last_sync", {
                  time: formatRelativeTime(syncLastAt),
                })}
          </Typography.Text>
        )}

        <RemoteConfigForm
          remoteUrl={remoteUrl}
          onRemoteUrlChange={setRemoteUrl}
          remoteUrlRef={remoteUrlRef}
          testing={testing}
          onTestConnection={() => void handleTestConnection()}
          sshHostsVisible={sshHostsVisible}
          sshHosts={sshHosts}
          onSshHostsChange={setSshHosts}
          isNoSync={isNoSync}
          onSaveConfig={() => void handleSaveConfig()}
        />

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
          <SyncActions
            syncing={syncing}
            pushBlocked={pendingRemovals !== null}
            onSyncNow={() => void syncNow()}
            onSyncPull={() => void syncPull()}
            onSyncPush={() => void syncPush()}
            onGoDangerZone={onGoDangerZone}
          />
        )}
      </Space>
    </Spin>
  );
}
