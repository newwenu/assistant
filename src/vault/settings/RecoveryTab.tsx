import { useState } from "react";
import { Button, Typography, Space, Modal, Input, App } from "antd";
import {
  CopyOutlined,
  KeyOutlined,
  SafetyCertificateOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import type { IntegrityResult } from "../core/vault.types";

interface RecoveryTabProps {
  recoveryKeySet: boolean;
  integrityResult: IntegrityResult | null;
  generateRecoveryKey: () => Promise<string | null>;
  checkRecoveryKeySet: () => Promise<void>;
  verifyIntegrity: () => Promise<IntegrityResult | null>;
}

export default function RecoveryTab({
  recoveryKeySet,
  integrityResult,
  generateRecoveryKey,
  checkRecoveryKeySet,
  verifyIntegrity,
}: RecoveryTabProps) {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const [recoveryKeyModalOpen, setRecoveryKeyModalOpen] = useState(false);
  const [generatedRecoveryKey, setGeneratedRecoveryKey] = useState<
    string | null
  >(null);
  const [recoveryKeyLoading, setRecoveryKeyLoading] = useState(false);

  const handleGenerateRecoveryKey = async () => {
    setRecoveryKeyLoading(true);
    try {
      const key = await generateRecoveryKey();
      if (key) {
        setGeneratedRecoveryKey(key);
        setRecoveryKeyModalOpen(true);
        await checkRecoveryKeySet();
      }
    } finally {
      setRecoveryKeyLoading(false);
    }
  };

  const handleCopyRecoveryKey = () => {
    if (!generatedRecoveryKey) return;
    void navigator.clipboard.writeText(generatedRecoveryKey);
    message.success(t("vault.recovery.key_copied"));
  };

  const handleConfirmSaved = () => {
    setRecoveryKeyModalOpen(false);
    setGeneratedRecoveryKey(null);
  };

  return (
    <>
      <div>
        <Typography.Paragraph type="secondary">
          {t("vault.recovery.key_description")}
        </Typography.Paragraph>

        <Space orientation="vertical" style={{ width: "100%" }}>
          <Button
            type="primary"
            icon={<KeyOutlined />}
            loading={recoveryKeyLoading}
            onClick={() => void handleGenerateRecoveryKey()}
          >
            {recoveryKeySet
              ? t("vault.recovery.key_regenerate")
              : t("vault.recovery.key_generate")}
          </Button>

          <Button
            icon={<SafetyCertificateOutlined />}
            onClick={() => void verifyIntegrity()}
          >
            {t("vault.recovery.verify_integrity")}
          </Button>

          {integrityResult && (
            <Typography.Text
              type={integrityResult.valid ? "success" : "danger"}
            >
              {integrityResult.message}
            </Typography.Text>
          )}
        </Space>
      </div>

      <Modal
        open={recoveryKeyModalOpen}
        title={t("vault.recovery.key_generated_title")}
        mask={{ closable: false }}
        onCancel={() => {
          setRecoveryKeyModalOpen(false);
          setGeneratedRecoveryKey(null);
        }}
        footer={[
          <Button
            key="copy"
            icon={<CopyOutlined />}
            disabled={!generatedRecoveryKey}
            onClick={handleCopyRecoveryKey}
          >
            {t("vault.recovery.key_copy")}
          </Button>,
          <Button key="saved" type="primary" onClick={handleConfirmSaved}>
            {t("vault.recovery.key_saved_confirm")}
          </Button>,
        ]}
      >
        <Typography.Paragraph type="warning">
          {t("vault.recovery.key_warning")}
        </Typography.Paragraph>
        {generatedRecoveryKey && (
          <Input.TextArea
            value={generatedRecoveryKey}
            readOnly
            autoSize
            style={{ fontFamily: "monospace", textAlign: "center" }}
          />
        )}
      </Modal>
    </>
  );
}
