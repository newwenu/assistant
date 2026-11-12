import { useState } from "react";
import { Alert, Button, Input, Space, Typography } from "antd";
import { WarningOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import styles from "../vault.module.css";

const RESET_TOKEN = "RESET-VAULT";

interface ResetRepositoryPanelProps {
  loading: boolean;
  error: string | null;
  onReset: (confirmToken: string) => Promise<void>;
}

export default function ResetRepositoryPanel({
  loading,
  error,
  onReset,
}: ResetRepositoryPanelProps) {
  const { t } = useTranslation();
  const [step, setStep] = useState<1 | 2>(1);
  const [token, setToken] = useState("");

  const tokenConfirmed = token.trim() === RESET_TOKEN;

  return (
    <div className={styles.unlockContainer}>
      <div className={styles.unlockCard}>
        <div className={styles.unlockIconWrap}>
          <WarningOutlined className={styles.unlockIcon} />
        </div>

        <Typography.Title level={4} style={{ margin: 0, textAlign: "center" }}>
          {t("vault.reset.title")}
        </Typography.Title>
        <Typography.Text type="secondary" className={styles.unlockSubtitle}>
          {t("vault.reset.desc")}
        </Typography.Text>

        <Alert
          type="warning"
          showIcon
          title={t("vault.reset.backupHint")}
          style={{ marginBottom: 16 }}
        />

        {step === 1 ? (
          <>
            <Typography.Paragraph type="danger" style={{ marginBottom: 16 }}>
              {t("vault.reset.confirm1")}
            </Typography.Paragraph>
            <Button
              danger
              type="primary"
              size="large"
              block
              onClick={() => setStep(2)}
            >
              {t("vault.reset.proceed")}
            </Button>
          </>
        ) : (
          <>
            <Typography.Paragraph type="danger" style={{ marginBottom: 12 }}>
              {t("vault.reset.confirm2")}
            </Typography.Paragraph>
            <Input
              size="large"
              value={token}
              placeholder={t("vault.reset.tokenLabel")}
              onChange={(e) => setToken(e.target.value)}
              status={token.length > 0 && !tokenConfirmed ? "error" : undefined}
            />
            {error ? (
              <Typography.Text type="danger" style={{ fontSize: 12 }}>
                {error}
              </Typography.Text>
            ) : null}
            <Space style={{ marginTop: 16, width: "100%" }}>
              <Button
                disabled={loading}
                onClick={() => {
                  setStep(1);
                  setToken("");
                }}
              >
                {t("common.actions.cancel")}
              </Button>
              <Button
                danger
                type="primary"
                disabled={!tokenConfirmed}
                loading={loading}
                onClick={() => void onReset(token.trim())}
              >
                {t("vault.reset.action")}
              </Button>
            </Space>
          </>
        )}
      </div>
    </div>
  );
}
