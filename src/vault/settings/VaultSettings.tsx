import { useState } from "react";
import {
  Button,
  Modal,
  Input,
  Tabs,
  Typography,
  Space,
  List,
  App,
  Radio,
  Alert,
} from "antd";
import {
  CloudUploadOutlined,
  DeleteOutlined,
  KeyOutlined,
  SafetyCertificateOutlined,
  DownloadOutlined,
  UploadOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useVaultStore } from "../core/vaultStore";
import VaultSyncSettings from "./VaultSyncSettings";

export default function VaultSettings() {
  const { t } = useTranslation();
  const { message } = App.useApp();
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
  const [recoveryKeyModalOpen, setRecoveryKeyModalOpen] = useState(false);
  const [generatedRecoveryKey, setGeneratedRecoveryKey] = useState<
    string | null
  >(null);
  const [recoveryKeyLoading, setRecoveryKeyLoading] = useState(false);
  const [exportFormat, setExportFormat] = useState<"json" | "csv">("json");
  const [importFormat, setImportFormat] = useState<"json" | "csv">("json");
  const [importFilePath, setImportFilePath] = useState("");

  const handleGenerateRecoveryKey = async () => {
    setRecoveryKeyLoading(true);
    try {
      const key = await generateRecoveryKey();
      if (key) {
        setGeneratedRecoveryKey(key);
        setRecoveryKeyModalOpen(true);
        await checkRecoveryKeySet();
      }
    } finally {
      setRecoveryKeyLoading(false);
    }
  };

  const handleSshAutoAddAll = async () => {
    const added = await sshAutoAddAll();
    if (added.length > 0) {
      message.success(
        t("tools.password_vault.ssh_auto_added", { count: added.length }),
      );
    } else {
      message.info(t("tools.password_vault.ssh_no_keys_to_add"));
    }
  };

  const sshAgentTab = (
    <div>
      <Typography.Paragraph type="secondary">
        {t("tools.password_vault.ssh_agent_description")}
      </Typography.Paragraph>

      <Space style={{ marginBottom: 8 }}>
        <Button
          icon={<CloudUploadOutlined />}
          onClick={() => void handleSshAutoAddAll()}
        >
          {t("tools.password_vault.ssh_add_all_to_agent")}
        </Button>
        <Button
          onClick={() => {
            void sshListAgent();
            void sshCheckAgentRunning();
          }}
        >
          {t("tools.password_vault.ssh_refresh_agent_keys")}
        </Button>
      </Space>

      <Typography.Text
        type="secondary"
        style={{ display: "block", marginBottom: 8 }}
      >
        {sshAgentRunning
          ? t("tools.password_vault.ssh_agent_running")
          : t("tools.password_vault.ssh_agent_not_running")}
      </Typography.Text>

      <List
        size="small"
        bordered
        dataSource={sshAgentKeys}
        renderItem={(key) => (
          <List.Item
            actions={[
              key.addedByVault && (
                <Button
                  key="remove"
                  type="text"
                  danger
                  size="small"
                  icon={<DeleteOutlined />}
                  onClick={() => void sshRemoveFromAgent(key.fingerprint)}
                />
              ),
            ].filter(Boolean)}
          >
            <List.Item.Meta
              title={`${key.keyType} ${key.fingerprint}`}
              description={`${key.comment} (${key.bits} bits)`}
            />
          </List.Item>
        )}
      />
    </div>
  );

  const recoveryTab = (
    <div>
      <Typography.Paragraph type="secondary">
        {t("tools.password_vault.recovery_key_description")}
      </Typography.Paragraph>

      <Space orientation="vertical" style={{ width: "100%" }}>
        <Button
          type="primary"
          icon={<KeyOutlined />}
          loading={recoveryKeyLoading}
          onClick={() => void handleGenerateRecoveryKey()}
        >
          {recoveryKeySet
            ? t("tools.password_vault.recovery_key_regenerate")
            : t("tools.password_vault.recovery_key_generate")}
        </Button>

        <Button
          icon={<SafetyCertificateOutlined />}
          onClick={() => void verifyIntegrity()}
        >
          {t("tools.password_vault.verify_integrity")}
        </Button>

        {integrityResult && (
          <Typography.Text type={integrityResult.valid ? "success" : "danger"}>
            {integrityResult.message}
          </Typography.Text>
        )}
      </Space>
    </div>
  );

  const handleExport = async () => {
    const filePath = await exportVault(exportFormat);
    if (filePath) {
      message.success(
        t("tools.password_vault.export_success", { path: filePath }),
      );
    }
  };

  const handleImport = async () => {
    if (!importFilePath.trim()) {
      message.warning(t("tools.password_vault.import_file_placeholder"));
      return;
    }
    const count = await importVault(importFilePath.trim(), importFormat);
    if (count !== null) {
      if (count > 0) {
        message.success(t("tools.password_vault.import_success", { count }));
      } else {
        message.info(t("tools.password_vault.import_no_entries"));
      }
      setImportFilePath("");
    }
  };

  const importExportTab = (
    <div>
      <Typography.Title level={5}>
        {t("tools.password_vault.export_title")}
      </Typography.Title>
      <Typography.Paragraph type="secondary">
        {t("tools.password_vault.export_description")}
      </Typography.Paragraph>

      <Alert
        type="warning"
        showIcon
        title={t("tools.password_vault.export_warning")}
        style={{ marginBottom: 16 }}
      />

      <Space orientation="vertical" style={{ width: "100%" }}>
        <Radio.Group
          value={exportFormat}
          onChange={(e) => setExportFormat(e.target.value as "json" | "csv")}
        >
          <Radio value="json">
            {t("tools.password_vault.export_format_json")}
          </Radio>
          <Radio value="csv">
            {t("tools.password_vault.export_format_csv")}
          </Radio>
        </Radio.Group>
        <Button
          type="primary"
          icon={<DownloadOutlined />}
          loading={loading}
          onClick={() => void handleExport()}
        >
          {t("tools.password_vault.export_button")}
        </Button>
      </Space>

      <Typography.Title level={5} style={{ marginTop: 24 }}>
        {t("tools.password_vault.import_title")}
      </Typography.Title>
      <Typography.Paragraph type="secondary">
        {t("tools.password_vault.import_description")}
      </Typography.Paragraph>

      <Space orientation="vertical" style={{ width: "100%" }}>
        <Radio.Group
          value={importFormat}
          onChange={(e) => setImportFormat(e.target.value as "json" | "csv")}
        >
          <Radio value="json">
            {t("tools.password_vault.import_format_json")}
          </Radio>
          <Radio value="csv">
            {t("tools.password_vault.import_format_csv")}
          </Radio>
        </Radio.Group>
        <Input
          value={importFilePath}
          onChange={(e) => setImportFilePath(e.target.value)}
          placeholder={t("tools.password_vault.import_file_placeholder")}
        />
        <Button
          type="primary"
          icon={<UploadOutlined />}
          loading={loading}
          disabled={!importFilePath.trim()}
          onClick={() => void handleImport()}
        >
          {t("tools.password_vault.import_button")}
        </Button>
      </Space>
    </div>
  );

  return (
    <>
      <Tabs
        items={[
          {
            key: "sync",
            label: t("tools.password_vault.tab_sync"),
            children: <VaultSyncSettings />,
          },
          {
            key: "ssh_agent",
            label: t("tools.password_vault.tab_ssh_agent"),
            children: sshAgentTab,
          },
          {
            key: "recovery",
            label: t("tools.password_vault.tab_recovery"),
            children: recoveryTab,
          },
          {
            key: "import_export",
            label: t("tools.password_vault.tab_import_export"),
            children: importExportTab,
          },
        ]}
      />

      <Modal
        open={recoveryKeyModalOpen}
        title={t("tools.password_vault.recovery_key_generated_title")}
        onCancel={() => {
          setRecoveryKeyModalOpen(false);
          setGeneratedRecoveryKey(null);
        }}
        footer={[
          <Button
            key="close"
            type="primary"
            onClick={() => {
              setRecoveryKeyModalOpen(false);
              setGeneratedRecoveryKey(null);
            }}
          >
            {t("common.actions.close")}
          </Button>,
        ]}
      >
        <Typography.Paragraph type="warning">
          {t("tools.password_vault.recovery_key_warning")}
        </Typography.Paragraph>
        {generatedRecoveryKey && (
          <Input.TextArea
            value={generatedRecoveryKey}
            readOnly
            autoSize
            style={{ fontFamily: "monospace", textAlign: "center" }}
          />
        )}
      </Modal>
    </>
  );
}
