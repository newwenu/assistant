import { useEffect, useState } from "react";
import { Button, Form, Typography } from "antd";
import { LockOutlined, SyncOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { save } from "@tauri-apps/plugin-dialog";
import { App } from "antd";
import { useVaultUnlock } from "./core/useVaultUnlock";
import PasswordForm from "./unlock/PasswordForm";
import BiometricButton from "./unlock/BiometricButton";
import DisasterRecovery from "./unlock/DisasterRecovery";
import styles from "./vault.module.css";

const BIOMETRIC_PROBE_TIMEOUT_MS = 2_000;

export default function VaultUnlock() {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const {
    isInitialized,
    loading,
    error,
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
    disasterRecoveryStep,
    disasterRecoveryResult,
    startDisasterRecovery,
    cancelDisasterRecovery,
    setDisasterRecoveryStep,
    disasterRecoveryExport,
    drGitUrl,
    setDrGitUrl,
    drRecoveryKey,
    setDrRecoveryKey,
    drCommit,
    setDrCommit,
  } = useVaultUnlock();

  const [biometricReady, setBiometricReady] = useState(false);

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

  const handleDisasterRecoveryNext = async () => {
    if (disasterRecoveryStep === 1) {
      setDisasterRecoveryStep(2);
    } else if (disasterRecoveryStep === 2) {
      if (!drRecoveryKey) return;
      const filePath = await save({
        defaultPath: "vault-recovery-export.json",
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!filePath) return;
      await disasterRecoveryExport(
        drGitUrl || null,
        drRecoveryKey,
        filePath,
        drCommit.trim() || null,
      );
    }
  };

  if (disasterRecoveryStep) {
    return (
      <DisasterRecovery
        step={disasterRecoveryStep}
        result={disasterRecoveryResult}
        loading={loading}
        error={error}
        drGitUrl={drGitUrl}
        onDrGitUrlChange={setDrGitUrl}
        drRecoveryKey={drRecoveryKey}
        onDrRecoveryKeyChange={setDrRecoveryKey}
        drCommit={drCommit}
        onDrCommitChange={setDrCommit}
        onNext={handleDisasterRecoveryNext}
        onCancel={cancelDisasterRecovery}
        onSetStep={setDisasterRecoveryStep}
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
          {biometricReady && biometricDefault ? (
            <BiometricButton
              unlockMethod={unlockMethod}
              onUnlock={() => void handleBiometricUnlock()}
              primary
            />
          ) : null}
          <PasswordForm
            isInitialized={isInitialized}
            password={password}
            onPasswordChange={setPassword}
            confirmPassword={confirmPassword}
            onConfirmPasswordChange={setConfirmPassword}
            strength={strength}
            error={error}
            unlockMethod={unlockMethod}
            canSubmit={canSubmit}
            collapsed={biometricDefault}
          />

          <div className={styles.unlockFooter}>
            <Button
              type="link"
              size="small"
              icon={<SyncOutlined />}
              disabled={!!unlockMethod}
              onClick={startDisasterRecovery}
            >
              {t("vault.disaster_recovery.title")}
            </Button>
          </div>
        </Form>
      </div>
    </div>
  );
}
