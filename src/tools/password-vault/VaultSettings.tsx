import { useState } from "react";
import {
  Form,
  InputNumber,
  Switch,
  Select,
  Button,
  Modal,
  Input,
  message,
} from "antd";
import { useTranslation } from "react-i18next";
import { useSettingsStore } from "@/stores/settingsStore";
import { settingService } from "@/services/settingService";
import { useVaultStore } from "./vaultStore";

export default function VaultSettings() {
  const { t } = useTranslation();
  const { appConf } = useSettingsStore();
  const { biometricAvailable, enrollBiometric, disenrollBiometric } =
    useVaultStore();
  const [enrollModalOpen, setEnrollModalOpen] = useState(false);
  const [enrollPassword, setEnrollPassword] = useState("");
  const [enrollLoading, setEnrollLoading] = useState(false);

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

  return (
    <>
      <Form
        layout="vertical"
        onFinish={(values) =>
          void handleSave(values as Record<string, unknown>)
        }
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
    </>
  );
}
