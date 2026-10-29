import { useState, useMemo, useCallback } from "react";
import { Modal, Input, Typography, Space } from "antd";
import { ExclamationCircleOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";

interface DangerZoneConfirmProps {
  open: boolean;
  title: string;
  description: string;
  confirmLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
  loading?: boolean;
}

function generateChallenge(length = 6): string {
  const chars = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
  let result = "";
  for (let i = 0; i < length; i++) {
    result += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  return result;
}

export default function DangerZoneConfirm({
  open,
  title,
  description,
  confirmLabel,
  onConfirm,
  onCancel,
  loading = false,
}: DangerZoneConfirmProps) {
  const { t } = useTranslation();
  const challenge = useMemo(() => generateChallenge(), [open]); // eslint-disable-line react-hooks/exhaustive-deps
  const [inputValue, setInputValue] = useState("");

  const isMatch = inputValue.toUpperCase() === challenge;

  const handleCancel = useCallback(() => {
    setInputValue("");
    onCancel();
  }, [onCancel]);

  const handleConfirm = useCallback(() => {
    if (!isMatch) return;
    setInputValue("");
    onConfirm();
  }, [isMatch, onConfirm]);

  return (
    <Modal
      open={open}
      title={
        <Space>
          <ExclamationCircleOutlined
            style={{ color: "var(--ant-color-error)" }}
          />
          {title}
        </Space>
      }
      onOk={handleConfirm}
      onCancel={handleCancel}
      okText={confirmLabel}
      okButtonProps={{ danger: true, disabled: !isMatch }}
      confirmLoading={loading}
      destroyOnHidden
    >
      <Space orientation="vertical" style={{ width: "100%" }} size="middle">
        <Typography.Text>{description}</Typography.Text>
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
    </Modal>
  );
}
