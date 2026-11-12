import { Button, Typography } from "antd";
import { InfoCircleOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import styles from "../vault.module.css";

interface VersionTooNewPanelProps {
  error: string | null;
  onRetry: () => void;
}

export default function VersionTooNewPanel({
  error,
  onRetry,
}: VersionTooNewPanelProps) {
  const { t } = useTranslation();

  return (
    <div className={styles.unlockContainer}>
      <div className={styles.unlockCard}>
        <div className={styles.unlockIconWrap}>
          <InfoCircleOutlined className={styles.unlockIcon} />
        </div>

        <Typography.Title level={4} style={{ margin: 0, textAlign: "center" }}>
          {t("vault.versionTooNew.title")}
        </Typography.Title>
        <Typography.Text type="secondary" className={styles.unlockSubtitle}>
          {t("vault.versionTooNew.desc")}
        </Typography.Text>

        {error ? (
          <Typography.Text type="danger" style={{ fontSize: 12 }}>
            {error}
          </Typography.Text>
        ) : null}

        <Button size="large" block onClick={onRetry} style={{ marginTop: 16 }}>
          {t("vault.versionTooNew.retry")}
        </Button>
      </div>
    </div>
  );
}
