import { useState, useEffect, useCallback } from "react";
import { Button, Space } from "antd";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { useVaultStore } from "@/vault/core/vaultStore";
import { vaultService } from "@/vault/core/vault.service";
import type { FieldRenderProps } from "../../core/types/fieldSchema";
import { useSettingMessage } from "../useSettingMessage";

export function BiometricSwitchRender({ value, onChange }: FieldRenderProps) {
  const { t } = useTranslation();
  const msg = useSettingMessage();
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
    const unlisten = listen("vault:biometric-enroll-step", () => {
      msg.info("settings.vault.biometric_enabled.enroll_step_hint");
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [msg]);

  const handleEnable = useCallback(async () => {
    if (!biometricAvailable) {
      msg.warning("vault.biometric.unavailable");
      return;
    }
    if (isLocked) {
      setLoading(true);
      const hide = msg.loading("vault.biometric.unlocking", 0);
      try {
        const ok = await vaultService.showUnlockWindow();
        hide();
        if (!ok) {
          msg.error("vault.biometric.unlock_failed", undefined);
          return;
        }
      } catch (e) {
        hide();
        msg.message.error(String(e));
        return;
      } finally {
        setLoading(false);
      }
    }
    setLoading(true);
    try {
      await enrollBiometric();
      onChange?.(true);
    } catch (e) {
      msg.message.error(String(e));
    } finally {
      setLoading(false);
    }
  }, [biometricAvailable, isLocked, enrollBiometric, onChange, msg]);

  const handleDisable = useCallback(async () => {
    setLoading(true);
    try {
      await disenrollBiometric();
      onChange?.(false);
    } catch (e) {
      msg.message.error(String(e));
    } finally {
      setLoading(false);
    }
  }, [disenrollBiometric, onChange, msg]);

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
