import { useEffect, useState, useCallback } from "react";
import {
  Button,
  App,
  Drawer,
  Typography,
  Space,
  Spin,
  Tooltip,
  Empty,
  Grid,
} from "antd";
import {
  LockOutlined,
  ToolOutlined,
  SafetyCertificateOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useVaultStore } from "./core/vaultStore";
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

export default function VaultApp() {
  const { t } = useTranslation();
  const { modal } = App.useApp();
  const statusReady = useVaultStatusReady();
  const {
    isLocked,
    entries,
    checkBiometricAvailable,
    checkBiometricEnrolled,
    lock,
    selectEntry,
  } = useVaultStore();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const screens = Grid.useBreakpoint();
  const isWide = screens.lg === true;
  const isDetail = !isWide && selectedId !== null;

  useEffect(() => {
    void checkBiometricAvailable();
    void checkBiometricEnrolled();
  }, [checkBiometricAvailable, checkBiometricEnrolled]);

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
      okType: "danger",
      onOk: () => {
        void lock();
        setSelectedId(null);
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

  const headerTitle = isDetail
    ? t("vault.entry.viewing_entry")
    : t("vault.tab.vault");

  return (
    <div className={styles.appContainer}>
      <div className={styles.appHeader}>
        <Typography.Text strong className={styles.headerTitle}>
          <SafetyCertificateOutlined style={{ marginRight: 6 }} />
          {headerTitle}
        </Typography.Text>
        <Space size={8}>
          <Button icon={<ToolOutlined />} onClick={() => setSettingsOpen(true)}>
            {t("vault.tab.tools")}
          </Button>
          <Tooltip title={t("vault.confirm_lock")}>
            <Button icon={<LockOutlined />} onClick={handleLock} />
          </Tooltip>
        </Space>
      </div>

      <div className={styles.appBody} data-wide={isWide ? "true" : "false"}>
        {(isWide || !isDetail) && (
          <div
            className={styles.listPane}
            data-wide={isWide ? "true" : "false"}
          >
            <VaultList
              onAdd={handleAdd}
              onEdit={handleEdit}
              onSelect={handleSelect}
              onDeleted={handleDeleted}
              selectedId={selectedId}
            />
            <VaultStatusBar entryCount={entries.length} />
          </div>
        )}

        {isWide ? (
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
        ) : (
          isDetail &&
          selectedId && (
            <div className={styles.detailPane} data-wide="false">
              <VaultEntryDetail
                entryId={selectedId}
                onEdit={handleEdit}
                onClose={handleClearSelection}
              />
            </div>
          )
        )}
      </div>

      <VaultEntryDrawer
        open={drawerOpen}
        editId={editingId}
        onClose={handleDrawerClose}
        onSaved={handleSaved}
      />

      <Drawer
        title={t("vault.tab.tools")}
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        size="large"
        destroyOnHidden
      >
        <VaultSettings />
      </Drawer>
    </div>
  );
}
