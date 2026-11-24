import React from "react";
import { Button, Modal } from "antd";
import { PlusOutlined, UnlockOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useChatStore } from "../../stores/chat/chatStore";
import css from "./CompactBanner.module.css";

export const CompactBanner: React.FC = () => {
  const { t } = useTranslation();
  const chat = useChatStore();
  const [modal, modalContextHolder] = Modal.useModal();

  if (!chat.isFull) return null;

  const handleNewConversation = () => {
    const currentConv = chat.conversations.find(
      (c) => c.id === chat.currentConversationId,
    );
    const providerId = currentConv?.providerId;
    const modelId = currentConv?.modelId;
    if (!providerId || !modelId) return;
    void chat.createConversation(providerId, modelId, "builtin-default");
  };

  const handleUnlock = () => {
    modal.confirm({
      title: t("chat.compact.unlock"),
      content: t("chat.compact.unlock_confirm"),
      onOk: () => {
        void chat.unlockConversation();
      },
    });
  };

  return (
    <div className={css.root}>
      {modalContextHolder}
      <div className={css.actions}>
        <Button
          size="small"
          type="primary"
          icon={<PlusOutlined />}
          onClick={handleNewConversation}
        >
          {t("chat.compact.new_conversation")}
        </Button>
        <Button size="small" icon={<UnlockOutlined />} onClick={handleUnlock}>
          {t("chat.compact.unlock")}
        </Button>
      </div>
    </div>
  );
};
