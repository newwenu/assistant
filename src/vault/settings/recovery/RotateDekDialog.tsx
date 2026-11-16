import { useCallback, useMemo, useState } from "react";
import {
  Alert,
  App,
  Button,
  Checkbox,
  Input,
  Modal,
  Space,
  Typography,
} from "antd";
import { ExclamationCircleOutlined, PrinterOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useVaultShallow, useVaultStore } from "../../core/vaultStore";
import { vaultErrorText } from "../../core/display/vaultErrorText";
import type { RecoveryKeyDto } from "../../core/vault.types";
import { printPaperKey } from "./printPaperKey";

interface RotateDekDialogProps {
  open: boolean;
  onClose: () => void;
}

function generateChallenge(length = 6): string {
  const chars = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
  let result = "";
  for (let i = 0; i < length; i++) {
    result += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  return result;
}

export default function RotateDekDialog({
  open,
  onClose,
}: RotateDekDialogProps) {
  const { t } = useTranslation();
  const { message, modal } = App.useApp();
  const { loading, error, errorCode, clearError, rotateDek } = useVaultShallow(
    (s) => ({
      loading: s.loading,
      error: s.error,
      errorCode: s.errorCode,
      clearError: s.clearError,
      rotateDek: s.rotateDek,
    }),
  );
  const { biometricAvailable, enrollBiometric } = useVaultShallow((s) => ({
    biometricAvailable: s.biometricAvailable,
    enrollBiometric: s.enrollBiometric,
  }));

  const challenge = useMemo(() => generateChallenge(), [open]); // eslint-disable-line react-hooks/exhaustive-deps
  const [inputValue, setInputValue] = useState("");
  const [result, setResult] = useState<RecoveryKeyDto | null>(null);
  const [written, setWritten] = useState(false);
  const [needsReenroll, setNeedsReenroll] = useState(false);

  const isMatch = inputValue.toUpperCase() === challenge;
  const errorText = vaultErrorText(errorCode, error);

  const reset = useCallback(() => {
    setInputValue("");
    setResult(null);
    setWritten(false);
    setNeedsReenroll(false);
    clearError();
  }, [clearError]);

  const handleRotate = async () => {
    if (!isMatch) return;
    // 轮换会删除本机凭据，注册提示只对「轮换前确实注册过」的用户有意义。
    const wasEnrolled = useVaultStore.getState().biometricEnrolled;
    const rotated = await rotateDek();
    if (!rotated) return;
    setResult(rotated);
    setWritten(false);
    setNeedsReenroll(wasEnrolled && biometricAvailable);
  };

  const promptReenroll = () => {
    modal.confirm({
      title: t("vault.recovery.rotate.reenroll_title"),
      content: t("vault.recovery.rotate.reenroll_desc"),
      okText: t("vault.recovery.rotate.reenroll_ok"),
      cancelText: t("vault.recovery.rotate.reenroll_skip"),
      onOk: async () => {
        try {
          await enrollBiometric();
          message.success(t("vault.recovery.rotate.reenroll_success"));
        } catch {
          message.error(t("vault.recovery.rotate.reenroll_failed"));
        }
      },
      onCancel: () => {
        message.info(t("vault.recovery.rotate.reenroll_disabled"));
      },
    });
  };

  const handlePrint = () => {
    if (!result) return;
    printPaperKey({
      title: t("vault.recovery.export.printTitle"),
      warning: t("vault.recovery.rotate.new_paper_warning"),
      groups: result.groups,
      meta: t("vault.recovery.export.dekVersion", {
        version: result.dekVersion,
      }),
      footer: t("vault.recovery.export.printFooter"),
    });
  };

  const handleClose = () => {
    const shouldPrompt = needsReenroll;
    reset();
    onClose();
    if (shouldPrompt) promptReenroll();
  };

  const warnings = [
    t("vault.recovery.rotate.warn_old_paper"),
    t("vault.recovery.rotate.warn_entries"),
    t("vault.recovery.rotate.warn_biometric"),
    t("vault.recovery.rotate.warn_history"),
  ];

  return (
    <Modal
      open={open}
      title={
        <Space>
          <ExclamationCircleOutlined
            style={{ color: "var(--ant-color-error)" }}
          />
          {t("vault.recovery.rotate.title")}
        </Space>
      }
      mask={{ closable: false }}
      onCancel={handleClose}
      destroyOnHidden
      footer={
        result
          ? [
              <Button
                key="print"
                icon={<PrinterOutlined />}
                onClick={handlePrint}
              >
                {t("vault.recovery.export.print")}
              </Button>,
              <Button
                key="done"
                type="primary"
                disabled={!written}
                onClick={handleClose}
              >
                {t("common.done")}
              </Button>,
            ]
          : [
              <Button key="cancel" onClick={handleClose}>
                {t("common.actions.cancel")}
              </Button>,
              <Button
                key="rotate"
                danger
                type="primary"
                loading={loading}
                disabled={!isMatch}
                onClick={() => void handleRotate()}
              >
                {t("vault.recovery.rotate.submit")}
              </Button>,
            ]
      }
    >
      {!result && (
        <Space orientation="vertical" style={{ width: "100%" }} size="middle">
          <Alert
            type="error"
            showIcon
            title={t("vault.recovery.rotate.warning")}
            description={
              <ul style={{ margin: 0, paddingInlineStart: 20 }}>
                {warnings.map((text) => (
                  <li key={text}>{text}</li>
                ))}
              </ul>
            }
          />
          <Typography.Text type="secondary">
            {t("vault.recovery.rotate.desc")}
          </Typography.Text>
          {errorText && <Alert type="error" showIcon title={errorText} />}
          <Typography.Text type="danger">
            {t("vault.danger_zone.type_confirm", { code: challenge })}
          </Typography.Text>
          <Input
            placeholder={challenge}
            value={inputValue}
            onChange={(e) => setInputValue(e.target.value)}
            autoFocus
            status={inputValue && !isMatch ? "error" : undefined}
          />
        </Space>
      )}

      {result && (
        <Space orientation="vertical" style={{ width: "100%" }} size="middle">
          <Alert
            type="warning"
            showIcon
            title={t("vault.recovery.rotate.new_paper_warning")}
          />
          <div
            style={{
              display: "flex",
              flexDirection: "column",
              gap: 8,
              fontFamily: "monospace",
              fontSize: 18,
              textAlign: "center",
            }}
          >
            {result.groups.map((group, index) => (
              <span key={index} style={{ letterSpacing: 2 }}>
                <span style={{ opacity: 0.5, marginRight: 8 }}>
                  {index + 1}
                </span>
                {group}
              </span>
            ))}
          </div>
          <Typography.Text type="secondary">
            {t("vault.recovery.export.dekVersion", {
              version: result.dekVersion,
            })}
          </Typography.Text>
          <Typography.Text type="secondary">
            {t("vault.recovery.rotate.biometric_notice")}
          </Typography.Text>
          <Typography.Text type="secondary" style={{ fontSize: 12 }}>
            {t("vault.recovery.export.channelNote")}
          </Typography.Text>
          <Checkbox
            checked={written}
            onChange={(e) => setWritten(e.target.checked)}
          >
            {t("vault.recovery.export.confirmWritten")}
          </Checkbox>
        </Space>
      )}
    </Modal>
  );
}
