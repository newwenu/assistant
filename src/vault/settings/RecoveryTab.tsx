import { useState } from "react";
import { Button, Typography, Space, Modal, Input } from "antd";
import { KeyOutlined, SafetyCertificateOutlined } from "@ant-design/icons";
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
        onCancel={() => {
          setRecoveryKeyModalOpen(false);
          setGeneratedRecoveryKey(null);
        }}
        footer={[
          <Button
            key="close"
            type="primary"
            onClick={() => {
              setRecoveryKeyModalOpen(false);
              setGeneratedRecoveryKey(null);
            }}
          >
            {t("common.actions.close")}
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
