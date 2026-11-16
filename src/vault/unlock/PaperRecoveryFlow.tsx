import { useMemo, useState } from "react";
import {
  Alert,
  App,
  Button,
  Input,
  Modal,
  Space,
  Steps,
  Typography,
} from "antd";
import { CopyOutlined, SafetyCertificateOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useVaultShallow, useVaultStore } from "../core/vaultStore";
import { vaultErrorText } from "../core/display/vaultErrorText";
import { normalizePaperInput, paperInputError } from "../core/display/paperKey";
import type { PaperErrorPosDto, RecoveryExportDto } from "../core/vault.types";
import RecoveryKeyForm from "../settings/recovery/RecoveryKeyForm";

interface PaperRecoveryFlowProps {
  open: boolean;
  onClose: () => void;
}

export default function PaperRecoveryFlow({
  open,
  onClose,
}: PaperRecoveryFlowProps) {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const {
    loading,
    error,
    errorCode,
    clearError,
    recoverFromPaper,
    locatePaperError,
  } = useVaultShallow((s) => ({
    loading: s.loading,
    error: s.error,
    errorCode: s.errorCode,
    clearError: s.clearError,
    recoverFromPaper: s.recoverFromPaper,
    locatePaperError: s.locatePaperError,
  }));

  const [raw, setRaw] = useState("");
  const [repoUrl, setRepoUrl] = useState("");
  const [errorPos, setErrorPos] = useState<PaperErrorPosDto | null>(null);
  const [result, setResult] = useState<RecoveryExportDto | null>(null);

  const normalized = useMemo(() => normalizePaperInput(raw), [raw]);
  const formatError = paperInputError(normalized);
  const errorText = vaultErrorText(errorCode, error);

  const reset = () => {
    setRaw("");
    setRepoUrl("");
    setErrorPos(null);
    setResult(null);
    clearError();
  };

  const handleClose = () => {
    reset();
    onClose();
  };

  const handleSubmit = async () => {
    if (formatError) return;
    setErrorPos(null);
    const exported = await recoverFromPaper(
      normalized,
      repoUrl.trim() || undefined,
    );
    if (exported) {
      setResult(exported);
      return;
    }
    const code = useVaultStore.getState().errorCode;
    if (code === "invalidRecoveryKeyChecksum") {
      const pos = await locatePaperError(normalized);
      setErrorPos(pos);
    }
  };

  const handleCopy = () => {
    if (!result) return;
    void navigator.clipboard.writeText(
      JSON.stringify(
        {
          exportedAt: result.exportedAt,
          dekVersion: result.dekVersion,
          entries: result.entries,
          skipped: result.skipped,
        },
        null,
        2,
      ),
    );
    message.success(t("vault.recovery.paper.copied"));
  };

  const step = result ? 2 : 0;

  return (
    <Modal
      open={open}
      title={t("vault.recovery.paper.title")}
      mask={{ closable: false }}
      onCancel={handleClose}
      destroyOnHidden
      width={560}
      footer={
        result
          ? [
              <Button key="copy" icon={<CopyOutlined />} onClick={handleCopy}>
                {t("vault.recovery.paper.copy")}
              </Button>,
              <Button key="done" type="primary" onClick={handleClose}>
                {t("common.done")}
              </Button>,
            ]
          : [
              <Button key="cancel" onClick={handleClose}>
                {t("common.actions.cancel")}
              </Button>,
              <Button
                key="submit"
                type="primary"
                icon={<SafetyCertificateOutlined />}
                loading={loading}
                disabled={formatError !== null}
                onClick={() => void handleSubmit()}
              >
                {t("vault.recovery.paper.submit")}
              </Button>,
            ]
      }
    >
      <Steps
        current={step}
        size="small"
        style={{ marginBottom: 24 }}
        items={[
          { title: t("vault.recovery.paper.step_input") },
          { title: t("vault.recovery.paper.step_done") },
        ]}
      />

      {!result && (
        <>
          <Typography.Paragraph type="secondary">
            {t("vault.recovery.paper.desc")}
          </Typography.Paragraph>
          <Space
            orientation="vertical"
            size={4}
            style={{ width: "100%", marginBottom: 16 }}
          >
            <Typography.Text>
              {t("vault.recovery.paper.repoUrl")}
            </Typography.Text>
            <Input
              value={repoUrl}
              onChange={(e) => setRepoUrl(e.target.value)}
              placeholder={t("vault.recovery.paper.repoUrlPlaceholder")}
              disabled={loading}
              allowClear
            />
            <Typography.Text type="secondary" style={{ fontSize: 12 }}>
              {t("vault.recovery.paper.repoUrlHint")}
            </Typography.Text>
          </Space>
          <RecoveryKeyForm
            value={normalized}
            onChange={setRaw}
            errorPos={errorPos}
            disabled={loading}
            autoFocus
          />
          {formatError && (
            <Alert
              type="warning"
              title={t(`vault.recovery.paper.format.${formatError}`)}
              showIcon
              style={{ marginTop: 16 }}
            />
          )}
          {errorText && (
            <Alert
              type="error"
              title={errorText}
              showIcon
              style={{ marginTop: 16 }}
            />
          )}
          {errorPos && (
            <Alert
              type="warning"
              title={t("vault.recovery.paper.locateResult", {
                group: errorPos.group,
                index: errorPos.index,
              })}
              showIcon
              style={{ marginTop: 16 }}
            />
          )}
        </>
      )}

      {result && (
        <>
          <Alert
            type="success"
            title={t("vault.recovery.paper.success", {
              count: result.entries.length,
            })}
            showIcon
            style={{ marginBottom: 16 }}
          />
          <Space orientation="vertical" size={8} style={{ width: "100%" }}>
            <Typography.Text type="secondary">
              {t("vault.recovery.export.dekVersion", {
                version: result.dekVersion,
              })}
            </Typography.Text>
            {result.skipped.length > 0 && (
              <Alert
                type="warning"
                title={t("vault.recovery.paper.skippedCount", {
                  count: result.skipped.length,
                })}
                showIcon
              />
            )}
            <Typography.Paragraph type="secondary" style={{ marginBottom: 0 }}>
              {t("vault.recovery.paper.nextSteps")}
            </Typography.Paragraph>
          </Space>
        </>
      )}
    </Modal>
  );
}
