import { useState } from "react";
import { Tabs } from "antd";
import { useTranslation } from "react-i18next";
import { useVaultShallow } from "../core/vaultStore";
import VaultSyncSettings from "./VaultSyncSettings";
import SshAgentTab from "./SshAgentTab";
import RecoveryTab from "./RecoveryTab";
import ImportExportTab from "./ImportExportTab";
import ChangePasswordTab from "./ChangePasswordTab";
import DangerZoneTab from "./DangerZoneTab";

export default function VaultSettings() {
  const { t } = useTranslation();
  const {
    sshAgentKeys,
    sshAgentRunning,
    loading,
    sshListAgent,
    sshCheckAgentRunning,
    sshRemoveFromAgent,
    sshAutoAddAll,
    exportVault,
    importVault,
  } = useVaultShallow((s) => ({
    sshAgentKeys: s.sshAgentKeys,
    sshAgentRunning: s.sshAgentRunning,
    loading: s.loading,
    sshListAgent: s.sshListAgent,
    sshCheckAgentRunning: s.sshCheckAgentRunning,
    sshRemoveFromAgent: s.sshRemoveFromAgent,
    sshAutoAddAll: s.sshAutoAddAll,
    exportVault: s.exportVault,
    importVault: s.importVault,
  }));
  const [activeKey, setActiveKey] = useState("sync");

  return (
    <Tabs
      activeKey={activeKey}
      onChange={setActiveKey}
      items={[
        {
          key: "sync",
          label: t("vault.tab.sync"),
          children: (
            <VaultSyncSettings
              onGoRecovery={() => setActiveKey("recovery")}
              onGoDangerZone={() => setActiveKey("danger_zone")}
            />
          ),
        },
        {
          key: "ssh_agent",
          label: t("vault.tab.ssh_agent"),
          children: (
            <SshAgentTab
              sshAgentKeys={sshAgentKeys}
              sshAgentRunning={sshAgentRunning}
              sshListAgent={sshListAgent}
              sshCheckAgentRunning={sshCheckAgentRunning}
              sshRemoveFromAgent={sshRemoveFromAgent}
              sshAutoAddAll={sshAutoAddAll}
            />
          ),
        },
        {
          key: "recovery",
          label: t("vault.tab.recovery"),
          children: <RecoveryTab />,
        },
        {
          key: "change_password",
          label: t("vault.tab.change_password"),
          children: <ChangePasswordTab />,
        },
        {
          key: "import_export",
          label: t("vault.tab.import_export"),
          children: (
            <ImportExportTab
              loading={loading}
              exportVault={exportVault}
              importVault={importVault}
            />
          ),
        },
        {
          key: "danger_zone",
          label: (
            <span style={{ color: "var(--ant-color-error)" }}>
              {t("vault.tab.danger_zone")}
            </span>
          ),
          children: <DangerZoneTab />,
        },
      ]}
    />
  );
}
