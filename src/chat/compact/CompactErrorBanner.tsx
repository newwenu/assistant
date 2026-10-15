import React from "react";
import { Button, Typography } from "antd";
import { CloseOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useChatStore } from "../../stores/chatStore";
import css from "./CompactErrorBanner.module.css";

export const CompactErrorBanner: React.FC = () => {
  const { t } = useTranslation();
  const chat = useChatStore();

  if (!chat.compactError) return null;

  return (
    <div className={css.root}>
      <Typography.Text type="danger">
        {t("chat.compact.error_title")}
      </Typography.Text>
      <Typography.Text type="secondary" style={{ fontSize: 12 }}>
        {chat.compactError}
      </Typography.Text>
      <Button
        size="small"
        type="link"
        onClick={() => {
          void chat.manualCompact();
        }}
      >
        {t("chat.compact.retry")}
      </Button>
      <Button
        size="small"
        type="link"
        icon={<CloseOutlined />}
        onClick={() => chat.clearCompactError()}
      />
    </div>
  );
};
