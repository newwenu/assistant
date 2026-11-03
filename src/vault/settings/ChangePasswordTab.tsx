import { useState, useCallback } from "react";
import { Form, Input, Button, Progress, Typography, Modal, App } from "antd";
import { LockOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useVaultShallow } from "../core/vaultStore";
import { vaultService } from "../core/vault.service";
import { vaultErrorMessage } from "../core/vaultError";
import type { StrengthResult } from "../core/vault.types";
import { STRENGTH_COLORS, STRENGTH_FALLBACK_COLOR } from "../core/vaultColors";

const STRENGTH_SUFFIXES = [
  "very_weak",
  "weak",
  "fair",
  "strong",
  "very_strong",
] as const;

const MIN_SCORE = 2;

export default function ChangePasswordTab() {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const {
    changeMasterPassword,
    loading,
    biometricAvailable,
    enrollBiometric,
    checkBiometricEnrolled,
  } = useVaultShallow((s) => ({
    changeMasterPassword: s.changeMasterPassword,
    loading: s.loading,
    biometricAvailable: s.biometricAvailable,
    enrollBiometric: s.enrollBiometric,
    checkBiometricEnrolled: s.checkBiometricEnrolled,
  }));

  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [strength, setStrength] = useState<StrengthResult | null>(null);

  const [reEnrollOpen, setReEnrollOpen] = useState(false);
  const [reEnrollLoading, setReEnrollLoading] = useState(false);

  const checkStrength = useCallback((pwd: string) => {
    if (pwd.length > 0) {
      void vaultService.checkMasterPasswordStrength(pwd).then((result) => {
        setStrength(result);
      });
    } else {
      setStrength(null);
    }
  }, []);

  const handleNewPasswordChange = useCallback(
    (value: string) => {
      setNewPassword(value);
      checkStrength(value);
    },
    [checkStrength],
  );

  const handleCurrentPasswordChange = useCallback((value: string) => {
    setCurrentPassword(value);
  }, []);

  const handleConfirmPasswordChange = useCallback((value: string) => {
    setConfirmPassword(value);
  }, []);

  const passwordMismatch =
    confirmPassword.length > 0 && newPassword !== confirmPassword;
  const tooWeak = newPassword.length > 0 && (strength?.score ?? 0) < MIN_SCORE;
  const canSubmit =
    !loading &&
    currentPassword.length > 0 &&
    newPassword.length >= 8 &&
    confirmPassword.length > 0 &&
    !passwordMismatch &&
    !tooWeak;

  const handleSubmit = async () => {
    try {
      await changeMasterPassword(currentPassword, newPassword);
      message.success(t("vault.change_password.success"));
      setCurrentPassword("");
      setNewPassword("");
      setConfirmPassword("");
      setStrength(null);
      if (biometricAvailable) {
        setReEnrollOpen(true);
      }
    } catch (e) {
      message.error(vaultErrorMessage(e));
    }
  };

  const handleReEnroll = async () => {
    setReEnrollLoading(true);
    try {
      await enrollBiometric();
      await checkBiometricEnrolled();
      message.success(t("vault.biometric.enabled"));
      setReEnrollOpen(false);
    } catch (e) {
      message.error(vaultErrorMessage(e));
    } finally {
      setReEnrollLoading(false);
    }
  };

  const handleReEnrollCancel = () => {
    setReEnrollOpen(false);
  };

  return (
    <div>
      <Typography.Paragraph type="secondary">
        {t("vault.change_password.description")}
      </Typography.Paragraph>

      <Form layout="vertical">
        <Form.Item label={t("vault.change_password.current")}>
          <Input.Password
            value={currentPassword}
            onChange={(e) => handleCurrentPasswordChange(e.target.value)}
            prefix={<LockOutlined />}
            autoFocus
          />
        </Form.Item>

        <Form.Item label={t("vault.change_password.new_password")}>
          <Input.Password
            value={newPassword}
            onChange={(e) => handleNewPasswordChange(e.target.value)}
            prefix={<LockOutlined />}
          />
        </Form.Item>

        {strength && (
          <div style={{ marginTop: -8, marginBottom: 16 }}>
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
            {tooWeak && (
              <Typography.Text type="danger" style={{ fontSize: 12 }}>
                {t("vault.strength.too_weak_hint")}
              </Typography.Text>
            )}
          </div>
        )}

        <Form.Item label={t("vault.change_password.confirm_new")}>
          <Input.Password
            value={confirmPassword}
            onChange={(e) => handleConfirmPasswordChange(e.target.value)}
            prefix={<LockOutlined />}
            status={passwordMismatch ? "error" : undefined}
          />
          {passwordMismatch && (
            <Typography.Text type="danger" style={{ fontSize: 12 }}>
              {t("vault.unlock.password_mismatch")}
            </Typography.Text>
          )}
        </Form.Item>

        <Form.Item>
          <Button
            type="primary"
            icon={<LockOutlined />}
            loading={loading}
            disabled={!canSubmit}
            onClick={() => void handleSubmit()}
          >
            {t("vault.change_password.submit")}
          </Button>
        </Form.Item>
      </Form>

      <Modal
        open={reEnrollOpen}
        title={t("vault.change_password.re_enroll_title")}
        onCancel={handleReEnrollCancel}
        footer={[
          <Button key="skip" onClick={handleReEnrollCancel}>
            {t("vault.change_password.re_enroll_skip")}
          </Button>,
          <Button
            key="enroll"
            type="primary"
            loading={reEnrollLoading}
            onClick={() => void handleReEnroll()}
          >
            {t("vault.change_password.re_enroll_confirm")}
          </Button>,
        ]}
        destroyOnHidden
      >
        <Typography.Paragraph type="secondary">
          {t("vault.change_password.re_enroll_desc")}
        </Typography.Paragraph>
      </Modal>
    </div>
  );
}
