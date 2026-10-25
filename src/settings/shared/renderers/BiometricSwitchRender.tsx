import { useState, useEffect, useCallback } from "react";
import { Button, App, Space } from "antd";
import { useTranslation } from "react-i18next";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useVaultStore } from "@/vault/core/vaultStore";
import { vaultService } from "@/vault/core/vault.service";
import type { FieldRenderProps } from "../../core/types/fieldSchema";

export function BiometricSwitchRender({ value, onChange }: FieldRenderProps) {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const {
    isLocked,
    biometricAvailable,
    enrollBiometric,
    disenrollBiometric,
    checkBiometricAvailable,
    checkBiometricEnrolled,
  } = useVaultStore();
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    void checkBiometricAvailable();
    void checkBiometricEnrolled();
  }, [checkBiometricAvailable, checkBiometricEnrolled]);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    void listen("vault:biometric-enroll-step", () => {
      message.info(t("settings.vault.biometric_enabled.enroll_step_hint"));
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [message, t]);

  const handleEnable = useCallback(async () => {
    if (!biometricAvailable) {
      message.warning(t("vault.biometric.unavailable"));
      return;
    }
    if (isLocked) {
      await vaultService.showUnlockWindow();
      return;
    }
    setLoading(true);
    try {
      await enrollBiometric();
      onChange?.(true);
    } catch (e) {
      message.error(String(e));
    } finally {
      setLoading(false);
    }
  }, [biometricAvailable, isLocked, enrollBiometric, onChange, message, t]);

  const handleDisable = useCallback(async () => {
    setLoading(true);
    try {
      await disenrollBiometric();
      onChange?.(false);
    } catch (e) {
      message.error(String(e));
    } finally {
      setLoading(false);
    }
  }, [disenrollBiometric, onChange, message]);

  if (value) {
    return (
      <Button
        type="primary"
        danger
        loading={loading}
        onClick={() => void handleDisable()}
      >
        {t("vault.biometric.disable")}
      </Button>
    );
  }

  return (
    <Space>
      <Button
        type="primary"
        loading={loading}
        onClick={() => void handleEnable()}
      >
        {t("vault.biometric.enable")}
      </Button>
    </Space>
  );
}
