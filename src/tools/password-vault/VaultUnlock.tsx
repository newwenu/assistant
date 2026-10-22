import { useState, useEffect } from "react";
import {
  Input,
  Button,
  Form,
  Typography,
  Progress,
  message,
  Divider,
} from "antd";
import {
  LockOutlined,
  UnlockOutlined,
  SafetyOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useVaultStore } from "./vaultStore";
import { vaultService } from "./vault.service";
import type { StrengthResult } from "./vault.service";
import styles from "./password-vault.module.css";

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

export default function VaultUnlock() {
  const { t } = useTranslation();
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [strength, setStrength] = useState<StrengthResult | null>(null);
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
  } = useVaultStore();

  useEffect(() => {
    if (!isInitialized && password.length > 0) {
      let cancelled = false;
      vaultService.checkMasterPasswordStrength(password).then((result) => {
        if (!cancelled) setStrength(result);
      });
      return () => {
        cancelled = true;
      };
    } else {
      setStrength(null);
    }
  }, [password, isInitialized]);

  const handleSubmit = async () => {
    clearError();
    if (!password) return;

    if (!isInitialized) {
      if (password !== confirmPassword) {
        message.error(t("tools.password_vault.password_mismatch"));
        return;
      }
      if (password.length < 8) {
        message.error(t("tools.password_vault.password_too_short"));
        return;
      }
    }

    if (!isInitialized) {
      await init(password);
    } else {
      await unlock(password);
    }
    setPassword("");
    setConfirmPassword("");
    setStrength(null);
  };

  const canSubmit =
    !loading &&
    password.length > 0 &&
    (isInitialized ||
      (password === confirmPassword && (strength?.score ?? 0) >= MIN_SCORE));

  return (
    <div className={styles.unlockContainer}>
      <LockOutlined className={styles.lockIcon} />
      <Typography.Title level={4}>
        {t(
          isInitialized
            ? "tools.password_vault.unlock_title"
            : "tools.password_vault.create_title",
        )}
      </Typography.Title>

      <Form onFinish={() => void handleSubmit()} className={styles.unlockForm}>
        <Form.Item>
          <Input.Password
            size="large"
            placeholder={t("tools.password_vault.master_password")}
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
                  `tools.password_vault.strength_${STRENGTH_SUFFIXES[strength.score] ?? "very_weak"}`,
                )}
              </Typography.Text>
              <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                {strength.crackTimeDisplay}
              </Typography.Text>
            </div>
            {strength.score < MIN_SCORE && (
              <Typography.Text type="danger" style={{ fontSize: 12 }}>
                {t("tools.password_vault.strength_too_weak_hint")}
              </Typography.Text>
            )}
          </div>
        )}

        {!isInitialized && (
          <Form.Item>
            <Input.Password
              size="large"
              placeholder={t("tools.password_vault.confirm_password")}
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
            loading={loading}
            disabled={!canSubmit}
          >
            {t(
              isInitialized
                ? "tools.password_vault.unlock"
                : "tools.password_vault.create_vault",
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
                loading={loading}
                onClick={() => void unlockBiometric()}
              >
                {t("tools.password_vault.biometric_unlock")}
              </Button>
            </Form.Item>
          </>
        )}
      </Form>
    </div>
  );
}
