import { useState } from "react";
import { Alert, App, Button, Checkbox, Input, Modal, Typography } from "antd";
import { KeyOutlined, PrinterOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useVaultShallow } from "../core/vaultStore";
import { vaultErrorText } from "../core/vaultErrorText";
import type { RecoveryKeyDto } from "../core/vault.types";
import { printPaperKey } from "./printPaperKey";

interface RecoveryKeyDialogProps {
  open: boolean;
  onClose: () => void;
}

export default function RecoveryKeyDialog({
  open,
  onClose,
}: RecoveryKeyDialogProps) {
  const { t } = useTranslation();
  const { modal } = App.useApp();
  const { loading, error, errorCode, clearError, exportRecoveryKey } =
    useVaultShallow((s) => ({
      loading: s.loading,
      error: s.error,
      errorCode: s.errorCode,
      clearError: s.clearError,
      exportRecoveryKey: s.exportRecoveryKey,
    }));

  const [masterPassword, setMasterPassword] = useState("");
  const [result, setResult] = useState<RecoveryKeyDto | null>(null);
  const [written, setWritten] = useState(false);

  const errorText = vaultErrorText(errorCode, error);

  const reset = () => {
    setMasterPassword("");
    setResult(null);
    setWritten(false);
    clearError();
  };

  const handleExport = async () => {
    const exported = await exportRecoveryKey(masterPassword);
    if (!exported) return;
    setMasterPassword("");
    setResult(exported);
    setWritten(false);
  };

  const handlePrint = () => {
    if (!result) return;
    printPaperKey({
      title: t("vault.recovery.export.printTitle"),
      warning: t("vault.recovery.export.warning"),
      groups: result.groups,
      meta: t("vault.recovery.export.dekVersion", {
        version: result.dekVersion,
      }),
      footer: t("vault.recovery.export.printFooter"),
    });
  };

  const handleClose = () => {
    if (result && !written) {
      modal.confirm({
        title: t("vault.recovery.export.confirmTitle"),
        content: t("vault.recovery.export.confirmWritten"),
        okText: t("common.done"),
        cancelText: t("common.actions.cancel"),
        onOk: () => {
          reset();
          onClose();
        },
      });
      return;
    }
    reset();
    onClose();
  };

  return (
    <Modal
      open={open}
      title={t("vault.recovery.export.title")}
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
                onClick={() => {
                  reset();
                  onClose();
                }}
              >
                {t("common.done")}
              </Button>,
            ]
          : [
              <Button key="cancel" onClick={handleClose}>
                {t("common.actions.cancel")}
              </Button>,
              <Button
                key="export"
                type="primary"
                loading={loading}
                disabled={masterPassword.length === 0}
                onClick={() => void handleExport()}
              >
                {t("vault.recovery.export.submit")}
              </Button>,
            ]
      }
    >
      {!result && (
        <>
          <Typography.Paragraph type="secondary">
            {t("vault.recovery.export.desc")}
          </Typography.Paragraph>
          {errorText && (
            <Alert
              type="error"
              title={errorText}
              showIcon
              style={{ marginBottom: 16 }}
            />
          )}
          <Input.Password
            size="large"
            prefix={<KeyOutlined />}
            placeholder={t("vault.change_password.current")}
            value={masterPassword}
            onChange={(e) => setMasterPassword(e.target.value)}
            autoFocus
          />
        </>
      )}

      {result && (
        <>
          <Alert
            type="warning"
            title={t("vault.recovery.export.warning")}
            showIcon
            style={{ marginBottom: 16 }}
          />
          <div
            style={{
              display: "flex",
              flexDirection: "column",
              gap: 8,
              fontFamily: "monospace",
              fontSize: 18,
              textAlign: "center",
              marginBottom: 16,
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
          <Typography.Text
            type="secondary"
            style={{ display: "block", marginBottom: 16 }}
          >
            {t("vault.recovery.export.dekVersion", {
              version: result.dekVersion,
            })}
          </Typography.Text>
          <Typography.Paragraph
            type="secondary"
            style={{ marginBottom: 16, fontSize: 12 }}
          >
            {t("vault.recovery.export.channelNote")}
          </Typography.Paragraph>
          <Checkbox
            checked={written}
            onChange={(e) => setWritten(e.target.checked)}
          >
            {t("vault.recovery.export.confirmWritten")}
          </Checkbox>
        </>
      )}
    </Modal>
  );
}
