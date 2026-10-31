import { useState } from "react";
import { Form, Input, Button, Progress, Typography, Divider } from "antd";
import { LockOutlined, UnlockOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import type { StrengthResult } from "../core/vault.types";
import { STRENGTH_COLORS, STRENGTH_FALLBACK_COLOR } from "../core/vaultColors";
import styles from "../vault.module.css";

const STRENGTH_SUFFIXES = [
  "very_weak",
  "weak",
  "fair",
  "strong",
  "very_strong",
] as const;

const MIN_SCORE = 2;

interface PasswordFormProps {
  isInitialized: boolean;
  password: string;
  onPasswordChange: (value: string) => void;
  confirmPassword: string;
  onConfirmPasswordChange: (value: string) => void;
  strength: StrengthResult | null;
  error: string | null;
  unlockMethod: "password" | "biometric" | null;
  canSubmit: boolean;
  collapsed?: boolean;
}

export default function PasswordForm({
  isInitialized,
  password,
  onPasswordChange,
  confirmPassword,
  onConfirmPasswordChange,
  strength,
  error,
  unlockMethod,
  canSubmit,
  collapsed = false,
}: PasswordFormProps) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);

  if (collapsed && !expanded) {
    return (
      <>
        <Divider plain>{t("common.or")}</Divider>
        <Button
          size="large"
          block
          icon={<LockOutlined />}
          disabled={!!unlockMethod}
          onClick={() => setExpanded(true)}
        >
          {t("vault.unlock.use_password")}
        </Button>
      </>
    );
  }

  return (
    <>
      {collapsed && (
        <Divider plain>{t("vault.unlock.password_divider")}</Divider>
      )}
      <Form.Item>
        <Input.Password
          size="large"
          placeholder={t("vault.unlock.master_password")}
          value={password}
          onChange={(e) => onPasswordChange(e.target.value)}
          prefix={<LockOutlined />}
          autoFocus
        />
      </Form.Item>

      {!isInitialized && strength && (
        <div style={{ marginBottom: 16 }}>
          <Progress
            percent={(strength.score / 4) * 100}
            strokeColor={
              STRENGTH_COLORS[strength.score] ?? STRENGTH_FALLBACK_COLOR
            }
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
            onChange={(e) => onConfirmPasswordChange(e.target.value)}
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
            isInitialized ? "vault.unlock.unlock" : "vault.unlock.create_vault",
          )}
        </Button>
      </Form.Item>
    </>
  );
}
