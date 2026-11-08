import { useEffect, useMemo, useRef, useState, useCallback } from "react";
import { Button, App, Drawer, Typography, Space, Spin, Empty } from "antd";
import {
  LockOutlined,
  SettingOutlined,
  SafetyCertificateOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useVaultShallow } from "./core/vaultStore";
import { useVaultStatusReady } from "./core/useVaultStatusReady";
import { useVaultEvents } from "./core/useVaultEvents";
import { useVaultAutoLock } from "./core/useVaultAutoLock";
import VaultUnlock from "./VaultUnlock";
import VaultList from "./VaultList";
import VaultStatusBar from "./VaultStatusBar";
import VaultEntryDrawer from "./VaultEntryDrawer";
import VaultEntryDetail from "./detail/VaultEntryDetail";
import VaultSettings from "./settings/VaultSettings";
import styles from "./vault.module.css";

const WIDE_LAYOUT_MIN_WIDTH = 784;

function formatRemaining(remainingMs: number): string {
  const totalSeconds = Math.max(0, Math.ceil(remainingMs / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${String(seconds).padStart(2, "0")}`;
}

export default function VaultApp() {
  const { t } = useTranslation();
  const { modal, message } = App.useApp();
  const statusReady = useVaultStatusReady();
  const {
    isLocked,
    entries,
    totalEntryCount,
    onlyLeaked,
    hibpLeaks,
    autoLockDeadline,
    checkBiometricAvailable,
    checkBiometricEnrolled,
    lock,
    selectEntry,
  } = useVaultShallow((s) => ({
    isLocked: s.isLocked,
    entries: s.entries,
    totalEntryCount: s.totalEntryCount,
    onlyLeaked: s.onlyLeaked,
    hibpLeaks: s.hibpLeaks,
    autoLockDeadline: s.autoLockDeadline,
    checkBiometricAvailable: s.checkBiometricAvailable,
    checkBiometricEnrolled: s.checkBiometricEnrolled,
    lock: s.lock,
    selectEntry: s.selectEntry,
  }));
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [containerWidth, setContainerWidth] = useState(0);
  const [remainingMs, setRemainingMs] = useState<number | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  const isWide = containerWidth >= WIDE_LAYOUT_MIN_WIDTH;
  const isDetail = !isWide && selectedId !== null;

  const visibleCount = useMemo(
    () =>
      onlyLeaked
        ? entries.filter((entry) => (hibpLeaks[entry.id] ?? 0) > 0).length
        : entries.length,
    [entries, onlyLeaked, hibpLeaks],
  );

  useEffect(() => {
    void checkBiometricAvailable();
    void checkBiometricEnrolled();
  }, [checkBiometricAvailable, checkBiometricEnrolled]);

  useEffect(() => {
    const element = containerRef.current;
    if (!element) return;
    setContainerWidth(element.clientWidth);
    const observer = new ResizeObserver((observed) => {
      const width = observed[0]?.contentRect.width;
      if (width !== undefined) setContainerWidth(width);
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [isLocked, statusReady]);

  useEffect(() => {
    if (autoLockDeadline === null) {
      setRemainingMs(null);
      return;
    }
    const tick = () =>
      setRemainingMs(Math.max(0, autoLockDeadline - Date.now()));
    tick();
    const timer = setInterval(tick, 1000);
    return () => clearInterval(timer);
  }, [autoLockDeadline]);

  useVaultEvents();
  useVaultAutoLock();

  const handleAdd = useCallback(() => {
    setEditingId(null);
    setDrawerOpen(true);
  }, []);

  const handleEdit = useCallback((id: string) => {
    setEditingId(id);
    setDrawerOpen(true);
  }, []);

  const handleSelect = useCallback(
    (id: string) => {
      setSelectedId(id);
      void selectEntry(id);
    },
    [selectEntry],
  );

  const handleSaved = useCallback((id: string) => {
    setSelectedId(id);
    setDrawerOpen(false);
    setEditingId(null);
  }, []);

  const handleDrawerClose = useCallback(() => {
    setDrawerOpen(false);
    setEditingId(null);
  }, []);

  const handleClearSelection = useCallback(() => {
    setSelectedId(null);
  }, []);

  const handleDeleted = useCallback((id: string) => {
    setSelectedId((current) => (current === id ? null : current));
  }, []);

  const handleLock = () => {
    modal.confirm({
      title: t("vault.confirm_lock"),
      onOk: async () => {
        try {
          await lock();
          setSelectedId(null);
        } catch {
          message.error(t("vault.lock_failed"));
        }
      },
    });
  };

  if (!statusReady) {
    return (
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          height: "100%",
        }}
      >
        <Spin size="large" />
      </div>
    );
  }

  if (isLocked) {
    return <VaultUnlock />;
  }

  return (
    <div ref={containerRef} className={styles.appContainer}>
      <div className={styles.appHeader}>
        <Typography.Title level={5} className={styles.headerTitle}>
          <SafetyCertificateOutlined style={{ marginRight: 6 }} />
          {t("vault.tab.vault")}
        </Typography.Title>
        <Space size={8}>
          {remainingMs !== null && (
            <Typography.Text type="secondary" className={styles.autoLockHint}>
              {t("vault.auto_lock_remaining", {
                time: formatRemaining(remainingMs),
              })}
            </Typography.Text>
          )}
          <Button
            icon={<SettingOutlined />}
            onClick={() => setSettingsOpen(true)}
          >
            {t("vault.tab.manage")}
          </Button>
          <Button
            icon={<LockOutlined />}
            aria-label={t("vault.lock")}
            onClick={handleLock}
          >
            {t("vault.lock")}
          </Button>
        </Space>
      </div>

      <div className={styles.appBody} data-wide={isWide ? "true" : "false"}>
        <div className={styles.listPane} data-wide={isWide ? "true" : "false"}>
          <div className={styles.listArea}>
            <VaultList
              onAdd={handleAdd}
              onEdit={handleEdit}
              onSelect={handleSelect}
              onDeleted={handleDeleted}
              onOpenManage={() => setSettingsOpen(true)}
              selectedId={selectedId}
            />
            {!isWide && isDetail && selectedId && (
              <div className={styles.detailOverlay}>
                <VaultEntryDetail
                  entryId={selectedId}
                  onEdit={handleEdit}
                  onClose={handleClearSelection}
                />
              </div>
            )}
          </div>
          <VaultStatusBar
            entryCount={visibleCount}
            totalCount={totalEntryCount}
            onOpenSettings={() => setSettingsOpen(true)}
          />
        </div>

        {isWide && (
          <div className={styles.detailPane} data-wide="true">
            {selectedId ? (
              <VaultEntryDetail
                entryId={selectedId}
                onEdit={handleEdit}
                onClose={handleClearSelection}
                showBack={false}
              />
            ) : (
              <div className={styles.detailEmpty}>
                <Empty
                  image={Empty.PRESENTED_IMAGE_SIMPLE}
                  description={t("vault.entry.select_hint")}
                />
              </div>
            )}
          </div>
        )}
      </div>

      <VaultEntryDrawer
        open={drawerOpen}
        editId={editingId}
        onClose={handleDrawerClose}
        onSaved={handleSaved}
      />

      <Drawer
        title={t("vault.tab.manage")}
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        size="min(480px, 100vw)"
        destroyOnHidden
      >
        <VaultSettings />
      </Drawer>
    </div>
  );
}
