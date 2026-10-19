import React from "react";
import { Modal, Button } from "antd";
import { useTranslation } from "react-i18next";

interface ForkDialogProps {
  open: boolean;
  onFork: () => void;
  onCancel: () => void;
}

export const ForkDialog: React.FC<ForkDialogProps> = ({
  open,
  onFork,
  onCancel,
}) => {
  const { t } = useTranslation();

  return (
    <Modal
      open={open}
      title={t("chat.branch.forkTitle")}
      onCancel={onCancel}
      footer={[
        <Button key="cancel" onClick={onCancel}>
          {t("common.actions.cancel")}
        </Button>,
        <Button key="fork" type="primary" onClick={onFork}>
          {t("chat.branch.forkAction")}
        </Button>,
      ]}
    >
      <p>{t("chat.branch.forkMessage")}</p>
      <p style={{ color: "var(--ant-color-text-secondary)", fontSize: 13 }}>
        {t("chat.branch.forkHint")}
      </p>
    </Modal>
  );
};
