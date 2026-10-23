import { useEffect } from "react";
import { Button, Divider, Form, Typography } from "antd";
import { LockOutlined, SyncOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { save } from "@tauri-apps/plugin-dialog";
import { App } from "antd";
import { useVaultUnlock } from "./core/useVaultUnlock";
import PasswordForm from "./unlock/PasswordForm";
import BiometricButton from "./unlock/BiometricButton";
import RecoveryKeyForm from "./unlock/RecoveryKeyForm";
import DisasterRecovery from "./unlock/DisasterRecovery";
import styles from "./vault.module.css";

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
    recoveryKey,
    setRecoveryKey,
    showRecoveryKey,
    setShowRecoveryKey,
    unlockMethod,
    canSubmit,
    biometricAvailable,
    biometricEnrolled,
    recoveryKeySet,
    handlePasswordSubmit,
    handleBiometricUnlock,
    handleRecoveryKeySubmit,
    checkBiometricAvailable,
    checkBiometricEnrolled,
    checkRecoveryKeySet,
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
  } = useVaultUnlock();

  useEffect(() => {
    void checkBiometricAvailable();
    void checkBiometricEnrolled();
  }, [checkBiometricAvailable, checkBiometricEnrolled]);

  useEffect(() => {
    if (!isInitialized && password.length > 0) {
      checkStrength(password);
    } else {
      setStrength(null);
    }
  }, [password, isInitialized, checkStrength, setStrength]);

  useEffect(() => {
    if (isInitialized) {
      void checkRecoveryKeySet();
    }
  }, [isInitialized, checkRecoveryKeySet]);

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
      await disasterRecoveryExport(drGitUrl || null, drRecoveryKey, filePath);
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
        onNext={handleDisasterRecoveryNext}
        onCancel={cancelDisasterRecovery}
        onSetStep={setDisasterRecoveryStep}
      />
    );
  }

  return (
    <div className={styles.unlockContainer}>
      <LockOutlined className={styles.lockIcon} />
      <Typography.Title level={4}>
        {t(isInitialized ? "vault.unlock.title" : "vault.unlock.create_title")}
      </Typography.Title>

      <Form
        onFinish={() => void handlePasswordSubmitInner()}
        className={styles.unlockForm}
      >
        {biometricDefault ? (
          <BiometricButton
            unlockMethod={unlockMethod}
            onUnlock={() => void handleBiometricUnlock()}
            primary
          />
        ) : (
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
          />
        )}

        {biometricDefault && (
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
            collapsed
          />
        )}

        {!biometricDefault &&
          isInitialized &&
          biometricAvailable &&
          biometricEnrolled && (
            <BiometricButton
              unlockMethod={unlockMethod}
              onUnlock={() => void handleBiometricUnlock()}
            />
          )}

        {isInitialized && recoveryKeySet && (
          <RecoveryKeyForm
            recoveryKey={recoveryKey}
            onRecoveryKeyChange={setRecoveryKey}
            showRecoveryKey={showRecoveryKey}
            onShowRecoveryKeyChange={setShowRecoveryKey}
            unlockMethod={unlockMethod}
            onSubmit={() => void handleRecoveryKeySubmit()}
          />
        )}

        <Divider plain>{t("common.or")}</Divider>
        <Form.Item>
          <Button
            size="large"
            block
            icon={<SyncOutlined />}
            disabled={!!unlockMethod}
            onClick={startDisasterRecovery}
          >
            {t("vault.disaster_recovery.title")}
          </Button>
        </Form.Item>
      </Form>
    </div>
  );
}
