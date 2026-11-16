import { useState, useCallback } from "react";
import { Button, Space, Typography } from "antd";
import { ExclamationCircleOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import DangerZoneConfirm from "./DangerZoneConfirm";
import styles from "../../vault.module.css";

export interface DangerZoneAction {
  key: string;
  title: string;
  description: string;
  buttonLabel: string;
  disabled?: boolean;
  loading?: boolean;
  onConfirm: () => Promise<void> | void;
}

interface DangerZoneProps {
  actions: DangerZoneAction[];
}

export default function DangerZone({ actions }: DangerZoneProps) {
  const { t } = useTranslation();
  const [pending, setPending] = useState<DangerZoneAction | null>(null);
  const [confirming, setConfirming] = useState(false);

  const handleConfirm = useCallback(async () => {
    if (!pending) return;
    setConfirming(true);
    try {
      await pending.onConfirm();
      setPending(null);
    } finally {
      setConfirming(false);
    }
  }, [pending]);

  const handleCancel = useCallback(() => {
    setPending(null);
  }, []);

  return (
    <>
      <div className={styles.dangerZone}>
        <Space orientation="vertical" style={{ width: "100%" }} size="middle">
          <div>
            <Typography.Text strong type="danger">
              <ExclamationCircleOutlined style={{ marginRight: 6 }} />
              {t("vault.danger_zone.title")}
            </Typography.Text>
            <Typography.Paragraph
              type="secondary"
              style={{ margin: "4px 0 0", fontSize: 12 }}
            >
              {t("vault.danger_zone.desc")}
            </Typography.Paragraph>
          </div>

          {actions.map((action) => (
            <div key={action.key} className={styles.dangerZoneAction}>
              <div className={styles.dangerZoneActionInfo}>
                <Typography.Text strong>{action.title}</Typography.Text>
                <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                  {action.description}
                </Typography.Text>
              </div>
              <Button
                danger
                disabled={action.disabled}
                loading={action.loading}
                onClick={() => setPending(action)}
              >
                {action.buttonLabel}
              </Button>
            </div>
          ))}
        </Space>
      </div>

      <DangerZoneConfirm
        open={pending !== null}
        title={pending?.title ?? ""}
        description={pending?.description ?? ""}
        confirmLabel={pending?.buttonLabel ?? ""}
        onConfirm={() => void handleConfirm()}
        onCancel={handleCancel}
        loading={confirming}
      />
    </>
  );
}
