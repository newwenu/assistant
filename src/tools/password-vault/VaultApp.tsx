import { useEffect, useState } from "react";
import { Tabs, Button, Modal } from "antd";
import { LockOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useVaultStore } from "./vaultStore";
import VaultUnlock from "./VaultUnlock";
import VaultList from "./VaultList";
import VaultEntryForm from "./VaultEntryForm";
import VaultEntryDetail from "./VaultEntryDetail";
import VaultSettings from "./VaultSettings";
import styles from "./password-vault.module.css";

type ViewMode = "list" | "detail" | "add" | "edit" | "settings";

export default function VaultApp() {
  const { t } = useTranslation();
  const { isLocked, checkStatus, lock, resetAutoLockTimer, selectEntry } =
    useVaultStore();
  const [view, setView] = useState<ViewMode>("list");
  const [selectedId, setSelectedId] = useState<string | null>(null);

  useEffect(() => {
    void checkStatus();
  }, [checkStatus]);

  useEffect(() => {
    let unlistenUnlock: UnlistenFn | undefined;
    let unlistenLock: UnlistenFn | undefined;

    void listen("vault:unlocked", () => {
      useVaultStore.setState({ isLocked: false });
      useVaultStore.getState().resetAutoLockTimer();
    }).then((fn) => {
      unlistenUnlock = fn;
    });

    void listen("vault:locked", () => {
      useVaultStore.setState({
        isLocked: true,
        entries: [],
        selectedEntry: null,
        revealedField: null,
      });
    }).then((fn) => {
      unlistenLock = fn;
    });

    return () => {
      unlistenUnlock?.();
      unlistenLock?.();
    };
  }, []);

  useEffect(() => {
    const handler = () => resetAutoLockTimer();
    window.addEventListener("mousemove", handler);
    window.addEventListener("keydown", handler);

    const visibilityHandler = () => {
      if (document.visibilityState === "visible") {
        resetAutoLockTimer();
      }
    };
    document.addEventListener("visibilitychange", visibilityHandler);

    return () => {
      window.removeEventListener("mousemove", handler);
      window.removeEventListener("keydown", handler);
      document.removeEventListener("visibilitychange", visibilityHandler);
    };
  }, [resetAutoLockTimer]);

  if (isLocked) {
    return <VaultUnlock />;
  }

  const handleAdd = () => {
    setSelectedId(null);
    setView("add");
  };

  const handleEdit = (id: string) => {
    void selectEntry(id);
    setSelectedId(id);
    setView("edit");
  };

  const handleSelect = (id: string) => {
    setSelectedId(id);
    setView("detail");
  };

  const handleSaved = (id: string) => {
    setSelectedId(id);
    setView("detail");
  };

  const handleCancel = () => {
    setSelectedId(null);
    setView("list");
  };

  const handleLock = () => {
    Modal.confirm({
      title: t("tools.password_vault.confirm_lock"),
      onOk: () => {
        void lock();
        setView("list");
        setSelectedId(null);
      },
    });
  };

  return (
    <div className={styles.appContainer}>
      <div className={styles.appHeader}>
        <Tabs
          activeKey={view === "settings" ? "settings" : "vault"}
          onChange={(key) => setView(key === "settings" ? "settings" : "list")}
          items={[
            { key: "vault", label: t("tools.password_vault.tab_vault") },
            { key: "settings", label: t("tools.password_vault.tab_settings") },
          ]}
          tabBarExtraContent={
            <Button type="text" icon={<LockOutlined />} onClick={handleLock}>
              {t("tools.password_vault.lock")}
            </Button>
          }
        />
      </div>

      <div className={styles.appBody}>
        {view === "settings" && <VaultSettings />}

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
            onClose={handleCancel}
          />
        )}

        {view === "add" && (
          <VaultEntryForm onCancel={handleCancel} onSaved={handleSaved} />
        )}

        {view === "edit" && selectedId && (
          <VaultEntryForm
            editId={selectedId}
            onCancel={handleCancel}
            onSaved={handleSaved}
          />
        )}
      </div>
    </div>
  );
}
