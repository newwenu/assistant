import { useState } from "react";
import {
  Form,
  InputNumber,
  Switch,
  Select,
  Button,
  Modal,
  Input,
  Tabs,
  Typography,
  Space,
  List,
  message,
} from "antd";
import {
  CloudUploadOutlined,
  DeleteOutlined,
  KeyOutlined,
  SafetyCertificateOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useSettingsStore } from "@/stores/settingsStore";
import { settingService } from "@/services/settingService";
import { useVaultStore } from "./vaultStore";
import VaultSyncSettings from "./VaultSyncSettings";

export default function VaultSettings() {
  const { t } = useTranslation();
  const { appConf } = useSettingsStore();
  const { biometricAvailable, enrollBiometric, disenrollBiometric } =
    useVaultStore();
  const {
    sshAgentKeys,
    sshAgentRunning,
    recoveryKeySet,
    integrityResult,
    sshListAgent,
    sshCheckAgentRunning,
    sshRemoveFromAgent,
    sshAutoAddAll,
    generateRecoveryKey,
    checkRecoveryKeySet,
    verifyIntegrity,
  } = useVaultStore();
  const [enrollModalOpen, setEnrollModalOpen] = useState(false);
  const [enrollPassword, setEnrollPassword] = useState("");
  const [enrollLoading, setEnrollLoading] = useState(false);
  const [recoveryKeyModalOpen, setRecoveryKeyModalOpen] = useState(false);
  const [generatedRecoveryKey, setGeneratedRecoveryKey] = useState<
    string | null
  >(null);
  const [recoveryKeyLoading, setRecoveryKeyLoading] = useState(false);

  const handleBiometricChange = (checked: boolean) => {
    if (checked) {
      if (!biometricAvailable) {
        message.warning(t("tools.password_vault.biometric_unavailable"));
        return;
      }
      setEnrollModalOpen(true);
      setEnrollPassword("");
    } else {
      void disenrollBiometric();
    }
  };

  const handleEnrollConfirm = async () => {
    if (!enrollPassword) return;
    setEnrollLoading(true);
    try {
      await enrollBiometric(enrollPassword);
      await settingService.patchConf("app", { vaultBiometricEnabled: true });
      setEnrollModalOpen(false);
      message.success(t("tools.password_vault.biometric_enabled"));
    } catch (e) {
      message.error(String(e));
    } finally {
      setEnrollLoading(false);
    }
  };

  const handleSave = async (values: Record<string, unknown>) => {
    try {
      await settingService.patchConf("app", values);
      message.success(t("common.actions.saved"));
    } catch {
      message.error("Save failed");
    }
  };

  const generalTab = (
    <Form
      layout="vertical"
      onFinish={(values) => void handleSave(values as Record<string, unknown>)}
      initialValues={{
        vaultAutoLockMinutes: appConf?.vaultAutoLockMinutes ?? 5,
        vaultLockOnScreenLock: appConf?.vaultLockOnScreenLock ?? true,
        vaultClipboardClearSeconds: appConf?.vaultClipboardClearSeconds ?? 30,
        vaultCipherAlgorithm: appConf?.vaultCipherAlgorithm ?? "aes256Gcm",
        vaultBiometricEnabled: appConf?.vaultBiometricEnabled ?? false,
        vaultHibpAutoAudit: appConf?.vaultHibpAutoAudit ?? true,
      }}
    >
      <Form.Item
        name="vaultAutoLockMinutes"
        label={t("tools.password_vault.settings_auto_lock")}
      >
        <InputNumber min={1} max={60} />
      </Form.Item>

      <Form.Item
        name="vaultLockOnScreenLock"
        label={t("tools.password_vault.settings_lock_on_screen_lock")}
        valuePropName="checked"
      >
        <Switch />
      </Form.Item>

      <Form.Item
        name="vaultClipboardClearSeconds"
        label={t("tools.password_vault.settings_clipboard_clear")}
      >
        <InputNumber min={5} max={300} />
      </Form.Item>

      <Form.Item
        name="vaultCipherAlgorithm"
        label={t("tools.password_vault.settings_cipher_algorithm")}
      >
        <Select
          options={[
            { value: "aes256Gcm", label: "AES-256-GCM" },
            { value: "chacha20Poly1305", label: "ChaCha20-Poly1305" },
          ]}
        />
      </Form.Item>

      <Form.Item
        name="vaultBiometricEnabled"
        label={t("tools.password_vault.settings_biometric")}
        valuePropName="checked"
      >
        <Switch onChange={handleBiometricChange} />
      </Form.Item>

      <Form.Item
        name="vaultHibpAutoAudit"
        label={t("tools.password_vault.settings_hibp_audit")}
        valuePropName="checked"
      >
        <Switch />
      </Form.Item>

      <Form.Item>
        <Button type="primary" htmlType="submit">
          {t("common.actions.save")}
        </Button>
      </Form.Item>
    </Form>
  );

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

      <Form
        layout="vertical"
        onFinish={(values) =>
          void handleSave(values as Record<string, unknown>)
        }
        initialValues={{
          vaultSshAutoAdd: appConf?.vaultSshAutoAdd ?? false,
          vaultSshUsePageant: appConf?.vaultSshUsePageant ?? true,
          vaultSshUseOpensshAgent: appConf?.vaultSshUseOpensshAgent ?? true,
          vaultSshAuthSockOverride: appConf?.vaultSshAuthSockOverride ?? "",
        }}
      >
        <Form.Item
          name="vaultSshAutoAdd"
          label={t("tools.password_vault.settings_ssh_auto_add")}
          valuePropName="checked"
        >
          <Switch />
        </Form.Item>

        {window.navigator.platform.toLowerCase().includes("win") && (
          <>
            <Form.Item
              name="vaultSshUsePageant"
              label={t("tools.password_vault.settings_ssh_use_pageant")}
              valuePropName="checked"
            >
              <Switch />
            </Form.Item>
            <Form.Item
              name="vaultSshUseOpensshAgent"
              label={t("tools.password_vault.settings_ssh_use_openssh_agent")}
              valuePropName="checked"
            >
              <Switch />
            </Form.Item>
          </>
        )}

        {!window.navigator.platform.toLowerCase().includes("win") && (
          <Form.Item
            name="vaultSshAuthSockOverride"
            label={t("tools.password_vault.settings_ssh_auth_sock_override")}
          >
            <Input
              placeholder={t(
                "tools.password_vault.settings_ssh_auth_sock_placeholder",
              )}
            />
          </Form.Item>
        )}

        <Form.Item>
          <Button type="primary" htmlType="submit">
            {t("common.actions.save")}
          </Button>
        </Form.Item>
      </Form>

      <div style={{ marginTop: 16 }}>
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

  return (
    <>
      <Tabs
        items={[
          {
            key: "general",
            label: t("tools.password_vault.tab_settings"),
            children: generalTab,
          },
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
        ]}
      />

      <Modal
        open={enrollModalOpen}
        title={t("tools.password_vault.biometric_enroll_title")}
        onOk={() => void handleEnrollConfirm()}
        onCancel={() => setEnrollModalOpen(false)}
        confirmLoading={enrollLoading}
        okButtonProps={{ disabled: !enrollPassword }}
      >
        <Input.Password
          placeholder={t("tools.password_vault.master_password")}
          value={enrollPassword}
          onChange={(e) => setEnrollPassword(e.target.value)}
          autoFocus
        />
      </Modal>

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
