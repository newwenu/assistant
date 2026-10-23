import { Space, Button, Typography, Tooltip } from "antd";
import { CopyOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import styles from "../vault.module.css";

interface TotpDisplayProps {
  entryId: string;
  code: string | null;
  timeRemaining: number;
  period: number;
  onCopy: (id: string) => void;
}

export default function TotpDisplay({
  entryId,
  code,
  timeRemaining,
  period,
  onCopy,
}: TotpDisplayProps) {
  const { t } = useTranslation();

  const radius = 18;
  const circumference = 2 * Math.PI * radius;
  const progress = period > 0 ? timeRemaining / period : 0;
  const dashOffset = circumference * (1 - progress);

  const formattedCode = code
    ? code.length === 6
      ? `${code.slice(0, 3)} ${code.slice(3)}`
      : code.length === 8
        ? `${code.slice(0, 4)} ${code.slice(4)}`
        : code
    : null;

  const isUrgent = timeRemaining <= 5 && timeRemaining > 0;

  return (
    <Space size="middle" align="center" className={styles.totpContainer}>
      <div className={styles.totpCountdownWrap}>
        <svg
          width={44}
          height={44}
          viewBox="0 0 44 44"
          className={styles.totpSvg}
        >
          <circle
            cx={22}
            cy={22}
            r={radius}
            fill="none"
            stroke="var(--ant-color-fill-secondary)"
            strokeWidth={3}
          />
          <circle
            cx={22}
            cy={22}
            r={radius}
            fill="none"
            stroke={
              isUrgent ? "var(--ant-color-error)" : "var(--ant-color-primary)"
            }
            strokeWidth={3}
            strokeDasharray={circumference}
            strokeDashoffset={dashOffset}
            strokeLinecap="round"
            transform={`rotate(-90 22 22)`}
            className={styles.totpArc}
          />
        </svg>
        <span className={styles.totpCountdownText}>{timeRemaining}</span>
      </div>
      {formattedCode ? (
        <Typography.Text
          code
          className={`${styles.totpCode} ${isUrgent ? styles.totpCodeUrgent : ""}`}
        >
          {formattedCode}
        </Typography.Text>
      ) : (
        <Typography.Text type="secondary">——</Typography.Text>
      )}
      <Tooltip title={t("vault.totp.copy")}>
        <Button
          type="text"
          size="small"
          icon={<CopyOutlined />}
          onClick={() => onCopy(entryId)}
        />
      </Tooltip>
    </Space>
  );
}
