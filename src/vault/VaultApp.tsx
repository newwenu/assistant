import { useEffect, useState, useCallback } from "react";
import { Button, App, Drawer, Typography, Space } from "antd";
import {
  LockOutlined,
  ToolOutlined,
  SafetyCertificateOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useVaultStore } from "./core/vaultStore";
import { useVaultEvents } from "./core/useVaultEvents";
import { useVaultAutoLock } from "./core/useVaultAutoLock";
import VaultUnlock from "./VaultUnlock";
import VaultList from "./VaultList";
import VaultEntryDrawer from "./VaultEntryDrawer";
import VaultEntryDetail from "./detail/VaultEntryDetail";
import VaultSettings from "./settings/VaultSettings";
import styles from "./vault.module.css";

type ViewMode = "list" | "detail";

export default function VaultApp() {
  const { t } = useTranslation();
  const { modal } = App.useApp();
  const {
    isLocked,
    checkStatus,
    checkBiometricAvailable,
    checkBiometricEnrolled,
    lock,
    selectEntry,
  } = useVaultStore();
  const [view, setView] = useState<ViewMode>("list");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);

  useEffect(() => {
    void checkStatus();
    void checkBiometricAvailable();
    void checkBiometricEnrolled();
  }, [checkStatus, checkBiometricAvailable, checkBiometricEnrolled]);

  useVaultEvents();
  useVaultAutoLock();

  const handleAdd = useCallback(() => {
    setEditingId(null);
    setDrawerOpen(true);
  }, []);

  const handleEdit = useCallback(
    (id: string) => {
      void selectEntry(id);
      setEditingId(id);
      setDrawerOpen(true);
    },
    [selectEntry],
  );

  const handleSelect = useCallback((id: string) => {
    setSelectedId(id);
    setView("detail");
  }, []);

  const handleSaved = useCallback((id: string) => {
    setSelectedId(id);
    setView("detail");
    setDrawerOpen(false);
    setEditingId(null);
  }, []);

  const handleDrawerClose = useCallback(() => {
    setDrawerOpen(false);
    setEditingId(null);
  }, []);

  const handleBackToList = useCallback(() => {
    setSelectedId(null);
    setView("list");
  }, []);

  const handleLock = () => {
    modal.confirm({
      title: t("vault.confirm_lock"),
      okType: "danger",
      onOk: () => {
        void lock();
        setView("list");
        setSelectedId(null);
      },
    });
  };

  if (isLocked) {
    return <VaultUnlock />;
  }

  const headerTitle =
    view === "detail"
      ? t("vault.entry.viewing_entry")
      : t("vault.tab.vault");

  return (
    <div className={styles.appContainer}>
      <div className={styles.appHeader}>
        <Typography.Text strong className={styles.headerTitle}>
          <SafetyCertificateOutlined style={{ marginRight: 6 }} />
          {headerTitle}
        </Typography.Text>
        <Space size={4}>
          <Button
            size="small"
            icon={<ToolOutlined />}
            onClick={() => setSettingsOpen(true)}
          >
            {t("vault.tab.tools")}
          </Button>
          <Button
            type="text"
            size="small"
            icon={<LockOutlined />}
            onClick={handleLock}
          />
        </Space>
      </div>

      <div className={styles.appBody}>
        {view === "list" && (
          <VaultList
            onAdd={handleAdd}
            onEdit={handleEdit}
            onSelect={handleSelect}
          />
        )}

        {view === "detail" && selectedId && (
          <VaultEntryDetail
            entryId={selectedId}
            onEdit={handleEdit}
            onClose={handleBackToList}
          />
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