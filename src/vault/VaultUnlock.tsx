import { useEffect, useState } from "react";
import { Form, Typography } from "antd";
import { LockOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { App } from "antd";
import { useVaultUnlock } from "./core/useVaultUnlock";
import {
  needsAppUpgrade,
  needsDbRepair,
  needsRepositoryReset,
} from "./core/vaultError";
import { vaultErrorText } from "./core/vaultErrorText";
import PasswordForm from "./unlock/PasswordForm";
import BiometricButton from "./unlock/BiometricButton";
import DisasterRecovery from "./unlock/DisasterRecovery";
import DbRepairDialog from "./unlock/DbRepairDialog";
import ResetRepositoryPanel from "./unlock/ResetRepositoryPanel";
import VersionTooNewPanel from "./unlock/VersionTooNewPanel";
import styles from "./vault.module.css";

const BIOMETRIC_PROBE_TIMEOUT_MS = 2_000;

export default function VaultUnlock() {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const {
    isInitialized,
    loading,
    error,
    errorCode,
    clearError,
    resetRepository,
    rebuildLocalDb,
    checkStatus,
    password,
    setPassword,
    confirmPassword,
    setConfirmPassword,
    strength,
    setStrength,
    checkStrength,
    unlockMethod,
    canSubmit,
    biometricAvailable,
    biometricEnrolled,
    handlePasswordSubmit,
    handleBiometricUnlock,
    checkBiometricAvailable,
    checkBiometricEnrolled,
  } = useVaultUnlock();

  const [biometricReady, setBiometricReady] = useState(false);
  const [repairDismissed, setRepairDismissed] = useState(false);
  const errorText = vaultErrorText(errorCode, error);

  useEffect(() => {
    if (!needsDbRepair(errorCode)) setRepairDismissed(false);
  }, [errorCode]);

  useEffect(() => {
    let cancelled = false;
    const timeoutId = setTimeout(() => {
      if (!cancelled) setBiometricReady(true);
    }, BIOMETRIC_PROBE_TIMEOUT_MS);

    void Promise.all([
      checkBiometricAvailable(),
      checkBiometricEnrolled(),
    ]).finally(() => {
      clearTimeout(timeoutId);
      if (!cancelled) setBiometricReady(true);
    });

    return () => {
      cancelled = true;
      clearTimeout(timeoutId);
    };
  }, [checkBiometricAvailable, checkBiometricEnrolled]);

  useEffect(() => {
    if (!isInitialized && password.length > 0) {
      checkStrength(password);
    } else {
      setStrength(null);
    }
  }, [password, isInitialized, checkStrength, setStrength]);

  const handlePasswordSubmitInner = async () => {
    if (!isInitialized) {
      if (password !== confirmPassword) {
        message.error(t("vault.unlock.password_mismatch"));
        return;
      }
      if (password.length < 8) {
        message.error(t("vault.unlock.password_too_short"));
        return;
      }
    }
    await handlePasswordSubmit();
  };

  const biometricDefault =
    isInitialized && biometricAvailable && biometricEnrolled;
  const biometricReset = errorCode === "biometricReset";

  if (needsRepositoryReset(errorCode)) {
    return (
      <ResetRepositoryPanel
        loading={loading}
        error={errorText}
        onReset={resetRepository}
      />
    );
  }

  if (needsAppUpgrade(errorCode)) {
    return (
      <VersionTooNewPanel
        error={errorText}
        onRetry={() => {
          clearError();
          void checkStatus();
        }}
      />
    );
  }

  return (
    <div className={styles.unlockContainer}>
      <div className={styles.unlockCard}>
        <div className={styles.unlockIconWrap}>
          <LockOutlined className={styles.unlockIcon} />
        </div>

        <Typography.Title level={4} style={{ margin: 0, textAlign: "center" }}>
          {t(
            isInitialized ? "vault.unlock.title" : "vault.unlock.create_title",
          )}
        </Typography.Title>
        <Typography.Text type="secondary" className={styles.unlockSubtitle}>
          {t(
            isInitialized
              ? "vault.unlock.subtitle"
              : "vault.unlock.create_subtitle",
          )}
        </Typography.Text>

        <Form
          onFinish={() => void handlePasswordSubmitInner()}
          className={styles.unlockForm}
        >
          {biometricReady && (biometricDefault || biometricReset) ? (
            <BiometricButton
              unlockMethod={unlockMethod}
              onUnlock={() => void handleBiometricUnlock()}
              primary={biometricDefault}
              resetRequired={biometricReset}
            />
          ) : null}
          <PasswordForm
            isInitialized={isInitialized}
            password={password}
            onPasswordChange={setPassword}
            confirmPassword={confirmPassword}
            onConfirmPasswordChange={setConfirmPassword}
            strength={strength}
            error={errorText}
            unlockMethod={unlockMethod}
            canSubmit={canSubmit}
            collapsed={biometricDefault}
          />

          <div className={styles.unlockFooter}>
            <DisasterRecovery disabled={!!unlockMethod} />
          </div>
        </Form>
      </div>

      <DbRepairDialog
        open={needsDbRepair(errorCode) && !repairDismissed}
        loading={loading}
        error={errorText}
        onRebuild={rebuildLocalDb}
        onClose={() => setRepairDismissed(true)}
      />
    </div>
  );
}
