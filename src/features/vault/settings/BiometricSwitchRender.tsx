import { useState, useEffect, useCallback } from "react";
import { Button, Space } from "antd";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { useVaultStore } from "@/vault/core/vaultStore";
import { vaultService } from "@/vault/core/vault.service";
import { vaultErrorMessage } from "@/vault/core/vaultError";
import { useSettingMessage } from "@/settings/shared/useSettingMessage";

export function BiometricSwitchRender() {
  const { t } = useTranslation();
  const msg = useSettingMessage();
  const {
    isLocked,
    biometricAvailable,
    biometricEnrolled,
    enrollBiometric,
    disenrollBiometric,
    checkBiometricAvailable,
    checkBiometricEnrolled,
  } = useVaultStore();
  const [loading, setLoading] = useState(false);
  const [checking, setChecking] = useState(true);

  useEffect(() => {
    let cancelled = false;
    void Promise.all([
      checkBiometricAvailable(),
      checkBiometricEnrolled(),
    ]).finally(() => {
      if (!cancelled) setChecking(false);
    });
    return () => {
      cancelled = true;
    };
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
        await vaultService.showUnlockWindow();
        hide();
      } catch (e) {
        hide();
        msg.message.error(vaultErrorMessage(e));
        return;
      } finally {
        setLoading(false);
      }
    }
    setLoading(true);
    try {
      await enrollBiometric();
      await checkBiometricEnrolled();
    } catch (e) {
      msg.message.error(vaultErrorMessage(e));
    } finally {
      setLoading(false);
    }
  }, [
    biometricAvailable,
    isLocked,
    enrollBiometric,
    checkBiometricEnrolled,
    msg,
  ]);

  const handleDisable = useCallback(async () => {
    if (isLocked) {
      msg.warning("vault.biometric.disable_requires_unlock");
      return;
    }
    setLoading(true);
    try {
      await disenrollBiometric();
      await checkBiometricEnrolled();
    } catch (e) {
      msg.message.error(vaultErrorMessage(e));
    } finally {
      setLoading(false);
    }
  }, [isLocked, disenrollBiometric, checkBiometricEnrolled, msg]);

  const busy = loading || checking;

  if (biometricEnrolled) {
    return (
      <Button
        type="primary"
        danger
        loading={busy}
        disabled={checking}
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
        loading={busy}
        disabled={checking}
        onClick={() => void handleEnable()}
      >
        {t("vault.biometric.enable")}
      </Button>
    </Space>
  );
}
