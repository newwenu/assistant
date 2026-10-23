import { useState, useEffect } from "react";
import { Switch, Modal, Input, App } from "antd";
import { useTranslation } from "react-i18next";
import { useVaultStore } from "@/vault/core/vaultStore";
import { settingService } from "@/services/settingService";
import type { FieldRenderProps } from "../../core/types/fieldSchema";

export function BiometricSwitchRender({ value, onChange }: FieldRenderProps) {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const {
    biometricAvailable,
    enrollBiometric,
    disenrollBiometric,
    checkBiometricAvailable,
    checkBiometricEnrolled,
  } = useVaultStore();
  const [enrollModalOpen, setEnrollModalOpen] = useState(false);
  const [enrollPassword, setEnrollPassword] = useState("");
  const [enrollLoading, setEnrollLoading] = useState(false);

  useEffect(() => {
    void checkBiometricAvailable();
    void checkBiometricEnrolled();
  }, [checkBiometricAvailable, checkBiometricEnrolled]);

  const handleChange = (checked: boolean) => {
    if (checked) {
      if (!biometricAvailable) {
        message.warning(t("vault.biometric.unavailable"));
        return;
      }
      setEnrollModalOpen(true);
      setEnrollPassword("");
    } else {
      void disenrollBiometric().then(() => {
        onChange?.(false);
      });
    }
  };

  const handleEnrollConfirm = async () => {
    if (!enrollPassword) return;
    setEnrollLoading(true);
    try {
      await enrollBiometric(enrollPassword);
      await settingService.patchConf("app", { vaultBiometricEnabled: true });
      onChange?.(true);
      setEnrollModalOpen(false);
      message.success(t("vault.biometric.enabled"));
    } catch (e) {
      message.error(String(e));
    } finally {
      setEnrollLoading(false);
    }
  };

  return (
    <>
      <Switch checked={!!value} onChange={handleChange} />
      <Modal
        open={enrollModalOpen}
        title={t("vault.biometric.enroll_title")}
        onOk={() => void handleEnrollConfirm()}
        onCancel={() => setEnrollModalOpen(false)}
        confirmLoading={enrollLoading}
        okButtonProps={{ disabled: !enrollPassword }}
      >
        <Input.Password
          placeholder={t("vault.unlock.master_password")}
          value={enrollPassword}
          onChange={(e) => setEnrollPassword(e.target.value)}
          autoFocus
        />
      </Modal>
    </>
  );
}