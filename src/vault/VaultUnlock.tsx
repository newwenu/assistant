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
  const state = useVaultUnlock();

  useEffect(() => {
    void state.checkBiometricAvailable();
    void state.checkBiometricEnrolled();
  }, [state.checkBiometricAvailable, state.checkBiometricEnrolled]);

  useEffect(() => {
    if (!state.isInitialized && state.password.length > 0) {
      state.checkStrength(state.password);
    } else {
      state.setStrength(null);
    }
  }, [state.password, state.isInitialized]);

  useEffect(() => {
    if (state.isInitialized) {
      void state.checkRecoveryKeySet();
    }
  }, [state.isInitialized, state.checkRecoveryKeySet]);

  const handlePasswordSubmit = async () => {
    if (!state.isInitialized) {
      if (state.password !== state.confirmPassword) {
        message.error(t("vault.unlock.password_mismatch"));
        return;
      }
      if (state.password.length < 8) {
        message.error(t("vault.unlock.password_too_short"));
        return;
      }
    }
    await state.handlePasswordSubmit();
  };

  const biometricDefault =
    state.isInitialized && state.biometricAvailable && state.biometricEnrolled;

  const handleDisasterRecoveryNext = async () => {
    if (state.disasterRecoveryStep === 1) {
      state.setDisasterRecoveryStep(2);
    } else if (state.disasterRecoveryStep === 2) {
      if (!state.drRecoveryKey) return;
      const filePath = await save({
        defaultPath: "vault-recovery-export.json",
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!filePath) return;
      await state.disasterRecoveryExport(
        state.drGitUrl || null,
        state.drRecoveryKey,
        filePath,
      );
    }
  };

  if (state.disasterRecoveryStep) {
    return (
      <DisasterRecovery
        step={state.disasterRecoveryStep}
        result={state.disasterRecoveryResult}
        loading={state.loading}
        error={state.error}
        drGitUrl={state.drGitUrl}
        onDrGitUrlChange={state.setDrGitUrl}
        drRecoveryKey={state.drRecoveryKey}
        onDrRecoveryKeyChange={state.setDrRecoveryKey}
        onNext={handleDisasterRecoveryNext}
        onCancel={state.cancelDisasterRecovery}
        onSetStep={state.setDisasterRecoveryStep}
      />
    );
  }

  return (
    <div className={styles.unlockContainer}>
      <LockOutlined className={styles.lockIcon} />
      <Typography.Title level={4}>
        {t(
          state.isInitialized
            ? "vault.unlock.title"
            : "vault.unlock.create_title",
        )}
      </Typography.Title>

      <Form
        onFinish={() => void handlePasswordSubmit()}
        className={styles.unlockForm}
      >
        {biometricDefault ? (
          <BiometricButton
            unlockMethod={state.unlockMethod}
            onUnlock={() => void state.handleBiometricUnlock()}
            primary
          />
        ) : (
          <PasswordForm
            isInitialized={state.isInitialized}
            password={state.password}
            onPasswordChange={state.setPassword}
            confirmPassword={state.confirmPassword}
            onConfirmPasswordChange={state.setConfirmPassword}
            strength={state.strength}
            error={state.error}
            unlockMethod={state.unlockMethod}
            canSubmit={state.canSubmit}
          />
        )}

        {biometricDefault && (
          <PasswordForm
            isInitialized={state.isInitialized}
            password={state.password}
            onPasswordChange={state.setPassword}
            confirmPassword={state.confirmPassword}
            onConfirmPasswordChange={state.setConfirmPassword}
            strength={state.strength}
            error={state.error}
            unlockMethod={state.unlockMethod}
            canSubmit={state.canSubmit}
            collapsed
          />
        )}

        {!biometricDefault &&
          state.isInitialized &&
          state.biometricAvailable &&
          state.biometricEnrolled && (
            <BiometricButton
              unlockMethod={state.unlockMethod}
              onUnlock={() => void state.handleBiometricUnlock()}
            />
          )}

        {state.isInitialized && state.recoveryKeySet && (
          <RecoveryKeyForm
            recoveryKey={state.recoveryKey}
            onRecoveryKeyChange={state.setRecoveryKey}
            showRecoveryKey={state.showRecoveryKey}
            onShowRecoveryKeyChange={state.setShowRecoveryKey}
            unlockMethod={state.unlockMethod}
            onSubmit={() => void state.handleRecoveryKeySubmit()}
          />
        )}

        <Divider plain>{t("common.or")}</Divider>
        <Form.Item>
          <Button
            size="large"
            block
            icon={<SyncOutlined />}
            disabled={!!state.unlockMethod}
            onClick={state.startDisasterRecovery}
          >
            {t("vault.disaster_recovery.title")}
          </Button>
        </Form.Item>
      </Form>
    </div>
  );
}
