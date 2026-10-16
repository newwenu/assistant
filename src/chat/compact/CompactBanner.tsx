import React from "react";
import { Button, Modal } from "antd";
import { PlusOutlined, UnlockOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useChatStore } from "../../stores/chatStore";
import { useProviderStore } from "../../stores/providerStore";
import css from "./CompactBanner.module.css";

export const CompactBanner: React.FC = () => {
  const { t } = useTranslation();
  const chat = useChatStore();
  const [modal, modalContextHolder] = Modal.useModal();

  if (!chat.isFull) return null;

  const handleNewConversation = () => {
    const provider = useProviderStore.getState().providers;
    if (provider.length === 0) return;
    const pid = provider[0].id;
    let modelId = provider[0].defaultModel || "";
    if (provider[0].models.length > 0 && !modelId)
      modelId = provider[0].models[0].id;
    void chat.createConversation(pid, modelId);
  };

  const handleUnlock = () => {
    modal.confirm({
      title: t("chat.compact.unlock"),
      content: t("chat.compact.unlock_confirm"),
      onOk: () => chat.unlockConversation(),
    });
  };

  return (
    <div className={css.root}>
      {modalContextHolder}
      <div className={css.actions}>
        <Button
          size="small"
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
