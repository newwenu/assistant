import { useState } from "react";
import { Alert, Input, Modal, Space, Typography } from "antd";
import { WarningOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";

interface DbRepairDialogProps {
  open: boolean;
  loading: boolean;
  error: string | null;
  onRebuild: (masterPassword: string) => Promise<void>;
  onClose: () => void;
}

export default function DbRepairDialog({
  open,
  loading,
  error,
  onRebuild,
  onClose,
}: DbRepairDialogProps) {
  const { t } = useTranslation();
  const [password, setPassword] = useState("");

  const handleClose = () => {
    setPassword("");
    onClose();
  };

  const handleRebuild = () => {
    if (!password) return;
    void onRebuild(password).then(() => setPassword(""));
  };

  return (
    <Modal
      open={open}
      title={
        <Space>
          <WarningOutlined style={{ color: "var(--ant-color-warning)" }} />
          {t("vault.repair.title")}
        </Space>
      }
      onOk={handleRebuild}
      onCancel={handleClose}
      okText={t("vault.repair.rebuildButton")}
      okButtonProps={{ danger: true, disabled: !password || loading }}
      confirmLoading={loading}
      destroyOnHidden
    >
      <Space orientation="vertical" style={{ width: "100%" }} size="middle">
        <Typography.Text>{t("vault.repair.desc")}</Typography.Text>
        <Alert
          type="warning"
          showIcon
          title={t("vault.repair.rebuildWarning")}
        />
        <Input.Password
          placeholder={t("vault.repair.passwordLabel")}
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          autoFocus
        />
        {error ? (
          <Typography.Text type="danger" style={{ fontSize: 12 }}>
            {error}
          </Typography.Text>
        ) : null}
      </Space>
    </Modal>
  );
}
