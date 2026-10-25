import { Tabs } from "antd";
import { useTranslation } from "react-i18next";
import { useVaultStore } from "../core/vaultStore";
import VaultSyncSettings from "./VaultSyncSettings";
import SshAgentTab from "./SshAgentTab";
import RecoveryTab from "./RecoveryTab";
import ImportExportTab from "./ImportExportTab";
import ChangePasswordTab from "./ChangePasswordTab";

export default function VaultSettings() {
  const { t } = useTranslation();
  const {
    sshAgentKeys,
    sshAgentRunning,
    recoveryKeySet,
    integrityResult,
    loading,
    sshListAgent,
    sshCheckAgentRunning,
    sshRemoveFromAgent,
    sshAutoAddAll,
    generateRecoveryKey,
    checkRecoveryKeySet,
    verifyIntegrity,
    exportVault,
    importVault,
  } = useVaultStore();

  return (
    <Tabs
      items={[
        {
          key: "sync",
          label: t("vault.tab.sync"),
          children: <VaultSyncSettings />,
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
          children: (
            <RecoveryTab
              recoveryKeySet={recoveryKeySet}
              integrityResult={integrityResult}
              generateRecoveryKey={generateRecoveryKey}
              checkRecoveryKeySet={checkRecoveryKeySet}
              verifyIntegrity={verifyIntegrity}
            />
          ),
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
      ]}
    />
  );
}
