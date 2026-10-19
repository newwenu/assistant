import React from "react";
import { Button } from "antd";
import { CloseOutlined } from "@ant-design/icons";
import { useChatStore } from "../../stores/chatStore";
import { ErrorBlock } from "../blocks/ErrorBlock";
import css from "./CompactErrorBanner.module.css";

export const CompactErrorBanner: React.FC = () => {
  const chat = useChatStore();

  if (!chat.compactError) return null;

  return (
    <div className={css.root}>
      <ErrorBlock
        error={chat.compactError}
        onRetry={() => {
          void chat.manualCompact();
        }}
      />
      <Button
        size="small"
        type="link"
        icon={<CloseOutlined />}
        onClick={() => chat.clearCompactError()}
      />
    </div>
  );
};
