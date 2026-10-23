import { useState, useEffect } from "react";
import {
  Input,
  Button,
  Form,
  Typography,
  Progress,
  App,
  Divider,
  Steps,
  Alert,
} from "antd";
import {
  LockOutlined,
  UnlockOutlined,
  SafetyOutlined,
  KeyOutlined,
  SyncOutlined,
  FolderOpenOutlined,
  CheckCircleOutlined,
  ArrowLeftOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { save } from "@tauri-apps/plugin-dialog";
import { useVaultStore } from "./core/vaultStore";
import { vaultService } from "./core/vault.service";
import type { StrengthResult } from "./core/vault.service";
import styles from "./vault.module.css";

const SCORE_COLORS: Record<number, string> = {
  0: "#ff4d4f",
  1: "#ff7a45",
  2: "#faad14",
  3: "#52c41a",
  4: "#13c8a8",
};

const STRENGTH_SUFFIXES = [
  "very_weak",
  "weak",
  "fair",
  "strong",
  "very_strong",
] as const;

const MIN_SCORE = 2;

type UnlockMethod = "password" | "biometric" | "recoveryKey" | null;

export default function VaultUnlock() {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [strength, setStrength] = useState<StrengthResult | null>(null);
  const [showRecoveryKey, setShowRecoveryKey] = useState(false);
  const [recoveryKey, setRecoveryKey] = useState("");
  const [unlockMethod, setUnlockMethod] = useState<UnlockMethod>(null);
  const [drGitUrl, setDrGitUrl] = useState("");
  const [drRecoveryKey, setDrRecoveryKey] = useState("");
  const {
    isInitialized,
    init,
    unlock,
    loading,
    error,
    clearError,
    biometricAvailable,
    biometricEnrolled,
    unlockBiometric,
    unlockWithRecoveryKey,
    checkRecoveryKeySet,
    checkBiometricAvailable,
    checkBiometricEnrolled,
    recoveryKeySet,
    disasterRecoveryStep,
    disasterRecoveryResult,
    startDisasterRecovery,
    cancelDisasterRecovery,
    setDisasterRecoveryStep,
    disasterRecoveryExport,
  } = useVaultStore();

  useEffect(() => {
    void checkBiometricAvailable();
    void checkBiometricEnrolled();
  }, [checkBiometricAvailable, checkBiometricEnrolled]);

  useEffect(() => {
    if (!isInitialized && password.length > 0) {
      let cancelled = false;
      void vaultService.checkMasterPasswordStrength(password).then((result) => {
        if (!cancelled) setStrength(result);
      });
      return () => {
        cancelled = true;
      };
    } else {
      setStrength(null);
    }
  }, [password, isInitialized]);

  useEffect(() => {
    if (isInitialized) {
      void checkRecoveryKeySet();
    }
  }, [isInitialized, checkRecoveryKeySet]);

  const handleSubmit = async () => {
    clearError();
    if (!password) return;

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

    setUnlockMethod("password");
    try {
      if (!isInitialized) {
        await init(password);
      } else {
        await unlock(password);
      }
    } finally {
      setUnlockMethod(null);
    }
    setPassword("");
    setConfirmPassword("");
    setStrength(null);
  };

  const handleBiometricUnlock = async () => {
    clearError();
    setUnlockMethod("biometric");
    try {
      await unlockBiometric();
    } finally {
      setUnlockMethod(null);
    }
  };

  const handleRecoveryKeySubmit = async () => {
    clearError();
    if (!recoveryKey) return;
    setUnlockMethod("recoveryKey");
    try {
      await unlockWithRecoveryKey(recoveryKey);
    } finally {
      setUnlockMethod(null);
    }
    setRecoveryKey("");
  };

  const canSubmit =
    !loading &&
    password.length > 0 &&
    (isInitialized ||
      (password === confirmPassword && (strength?.score ?? 0) >= MIN_SCORE));

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
    const stepLabels = [
      t("vault.disaster_recovery.step_git"),
      t("vault.disaster_recovery.step_key"),
      t("vault.disaster_recovery.step_done"),
    ];

    return (
      <div className={styles.unlockContainer}>
        <SyncOutlined className={styles.lockIcon} spin={loading} />
        <Typography.Title level={4}>
          {t("vault.disaster_recovery.title")}
        </Typography.Title>

        <Steps
          current={disasterRecoveryStep - 1}
          size="small"
          style={{ marginBottom: 24 }}
          items={stepLabels.map((label) => ({ title: label }))}
        />

        {error && (
          <Alert
            type="error"
            title={error}
            showIcon
            style={{ marginBottom: 16 }}
          />
        )}

        {disasterRecoveryStep === 1 && (
          <Form
            onFinish={() => void handleDisasterRecoveryNext()}
            className={styles.unlockForm}
          >
            <Typography.Text
              type="secondary"
              style={{ marginBottom: 8, display: "block" }}
            >
              {t("vault.disaster_recovery.git_hint")}
            </Typography.Text>
            <Form.Item>
              <Input
                size="large"
                placeholder={t(
                  "vault.disaster_recovery.git_placeholder",
                )}
                value={drGitUrl}
                onChange={(e) => setDrGitUrl(e.target.value)}
                autoFocus
              />
            </Form.Item>
            <Form.Item>
              <Button
                type="primary"
                size="large"
                block
                htmlType="submit"
                disabled={loading}
                loading={loading}
              >
                {t("common.next")}
              </Button>
            </Form.Item>
          </Form>
        )}

        {disasterRecoveryStep === 2 && (
          <Form
            onFinish={() => void handleDisasterRecoveryNext()}
            className={styles.unlockForm}
          >
            <Form.Item>
              <Input
                size="large"
                placeholder={t("vault.unlock.recovery_key_placeholder")}
                value={drRecoveryKey}
                onChange={(e) => setDrRecoveryKey(e.target.value)}
                prefix={<KeyOutlined />}
                autoFocus
              />
            </Form.Item>
            <Form.Item>
              <Button
                type="primary"
                size="large"
                block
                htmlType="submit"
                icon={<SyncOutlined />}
                disabled={!drRecoveryKey || loading}
                loading={loading}
              >
                {t("vault.disaster_recovery.export")}
              </Button>
            </Form.Item>
          </Form>
        )}

        {disasterRecoveryStep === 3 && disasterRecoveryResult && (
          <div className={styles.unlockForm}>
            {disasterRecoveryResult.integrityWarning && (
              <Alert
                type="warning"
                title={t(
                  "vault.disaster_recovery.integrity_warning",
                )}
                showIcon
                style={{ marginBottom: 16 }}
              />
            )}
            <Alert
              type="success"
              title={t("vault.disaster_recovery.success", {
                count: disasterRecoveryResult.entryCount,
              })}
              showIcon
              style={{ marginBottom: 16 }}
            />
            <Typography.Text
              type="secondary"
              style={{ display: "block", marginBottom: 8 }}
            >
              {t("vault.disaster_recovery.next_steps")}
            </Typography.Text>
            <Button
              size="large"
              block
              icon={<FolderOpenOutlined />}
              style={{ marginBottom: 8 }}
              onClick={() => {
                void save({
                  defaultPath: disasterRecoveryResult.filePath,
                });
              }}
            >
              {t("vault.disaster_recovery.open_file")}
            </Button>
            <Button
              type="primary"
              size="large"
              block
              icon={<CheckCircleOutlined />}
              onClick={cancelDisasterRecovery}
            >
              {t("common.done")}
            </Button>
          </div>
        )}

        <Button
          type="link"
          icon={<ArrowLeftOutlined />}
          onClick={() => {
            if (disasterRecoveryStep === 1) {
              cancelDisasterRecovery();
            } else {
              setDisasterRecoveryStep((disasterRecoveryStep - 1) as 1 | 2);
            }
          }}
          style={{ marginTop: 16 }}
        >
          {t("common.back")}
        </Button>
      </div>
    );
  }

  return (
    <div className={styles.unlockContainer}>
      <LockOutlined className={styles.lockIcon} />
      <Typography.Title level={4}>
        {t(
          isInitialized
            ? "vault.unlock.title"
            : "vault.unlock.create_title",
        )}
      </Typography.Title>

      <Form onFinish={() => void handleSubmit()} className={styles.unlockForm}>
        <Form.Item>
          <Input.Password
            size="large"
            placeholder={t("vault.unlock.master_password")}
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            prefix={<LockOutlined />}
            autoFocus
          />
        </Form.Item>

        {!isInitialized && strength && (
          <div style={{ marginBottom: 16 }}>
            <Progress
              percent={(strength.score / 4) * 100}
              strokeColor={SCORE_COLORS[strength.score] ?? "#d9d9d9"}
              showInfo={false}
              size="small"
            />
            <div
              style={{
                display: "flex",
                justifyContent: "space-between",
                marginTop: 4,
              }}
            >
              <Typography.Text
                type={strength.score >= MIN_SCORE ? "success" : "danger"}
                style={{ fontSize: 12 }}
              >
                {t(
                  `vault.strength.${STRENGTH_SUFFIXES[strength.score] ?? "very_weak"}`,
                )}
              </Typography.Text>
              <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                {strength.crackTimeDisplay}
              </Typography.Text>
            </div>
            {strength.score < MIN_SCORE && (
              <Typography.Text type="danger" style={{ fontSize: 12 }}>
                {t("vault.strength.too_weak_hint")}
              </Typography.Text>
            )}
          </div>
        )}

        {!isInitialized && (
          <Form.Item>
            <Input.Password
              size="large"
              placeholder={t("vault.unlock.confirm_password")}
              value={confirmPassword}
              onChange={(e) => setConfirmPassword(e.target.value)}
              prefix={<LockOutlined />}
            />
          </Form.Item>
        )}

        {error && (
          <Typography.Text type="danger" className={styles.errorText}>
            {error}
          </Typography.Text>
        )}

        <Form.Item>
          <Button
            type="primary"
            htmlType="submit"
            size="large"
            block
            icon={<UnlockOutlined />}
            loading={unlockMethod === "password"}
            disabled={!canSubmit}
          >
            {t(
              isInitialized
                ? "vault.unlock.unlock"
                : "vault.unlock.create_vault",
            )}
          </Button>
        </Form.Item>

        {isInitialized && biometricAvailable && biometricEnrolled && (
          <>
            <Divider plain>{t("common.or")}</Divider>
            <Form.Item>
              <Button
                size="large"
                block
                icon={<SafetyOutlined />}
                loading={unlockMethod === "biometric"}
                disabled={!!unlockMethod}
                onClick={() => void handleBiometricUnlock()}
              >
                {t("vault.unlock.biometric_unlock")}
              </Button>
            </Form.Item>
          </>
        )}

        {isInitialized && recoveryKeySet && !showRecoveryKey && (
          <>
            <Divider plain>{t("common.or")}</Divider>
            <Form.Item>
              <Button
                size="large"
                block
                icon={<KeyOutlined />}
                disabled={!!unlockMethod}
                onClick={() => setShowRecoveryKey(true)}
              >
                {t("vault.unlock.recovery_key_unlock")}
              </Button>
            </Form.Item>
          </>
        )}

        {isInitialized && showRecoveryKey && (
          <>
            <Divider plain>
              {t("vault.unlock.recovery_key_divider")}
            </Divider>
            <Form.Item>
              <Input
                size="large"
                placeholder={t("vault.unlock.recovery_key_placeholder")}
                value={recoveryKey}
                onChange={(e) => setRecoveryKey(e.target.value)}
                prefix={<KeyOutlined />}
                autoFocus
              />
            </Form.Item>
            <Form.Item>
              <Button
                type="primary"
                size="large"
                block
                icon={<UnlockOutlined />}
                loading={unlockMethod === "recoveryKey"}
                disabled={!recoveryKey || !!unlockMethod}
                onClick={() => void handleRecoveryKeySubmit()}
              >
                {t("vault.unlock.unlock_with_recovery_key")}
              </Button>
            </Form.Item>
          </>
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