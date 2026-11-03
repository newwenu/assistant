import React, { useEffect, useState, useCallback, type RefObject } from "react";
import { Button, Modal, Typography, Tooltip } from "antd";
import { ContextBar } from "./context/ContextBar";
import {
  resolveDefaultModel,
  resolveProviderModel,
} from "./core/providerModel";
import {
  PlusOutlined,
  SettingOutlined,
  PaperClipOutlined,
  ThunderboltOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { chatService } from "../services/chatService";
import { orbService } from "../services/orbService";
import { useChatStore } from "../stores/chatStore";
import { useChatStreamStore } from "../stores/chatStreamStore";
import { useProviderStore } from "../stores/providerStore";
import { MessageList } from "./message/MessageList";
import { MessageInput } from "./message/MessageInput";
import { ModelSelector } from "./selectors/ModelSelector";
import { ModeSelector } from "./selectors/ModeSelector";
import { ReasoningSelector } from "./selectors/ReasoningSelector";
import { ProviderSelector } from "./selectors/ProviderSelector";
import { CompactBanner } from "./compact/CompactBanner";
import { CompactErrorBanner } from "./compact/CompactErrorBanner";
import type { RecoveryActionCode } from "./core/chatErrorRecovery";
import { useAgentModeStore } from "../stores/agentModeStore";
import { useAttachmentStore } from "../stores/attachmentStore";
import { AttachmentPreview } from "./message/AttachmentPreview";
import { ForkDialog } from "./blocks/ForkDialog";
import { FeatureErrorBoundary } from "@/components/FeatureErrorBoundary";
import { createLogger } from "@/services/logger";
import { useAttachmentIntake } from "./attachment";

const log = createLogger("ChatView");

interface ChatViewProps {
  modelOpenRef?: RefObject<boolean>;
}

const AttachmentPreviewBar: React.FC = () => {
  const attachments = useAttachmentStore((s) => s.attachments);
  const removeAttachment = useAttachmentStore((s) => s.removeAttachment);
  return (
    <AttachmentPreview attachments={attachments} onRemove={removeAttachment} />
  );
};

export const ChatView: React.FC<ChatViewProps> = ({ modelOpenRef }) => {
  const chat = useChatStore();
  const stream = useChatStreamStore();
  const provider = useProviderStore();
  const [modal, modalContextHolder] = Modal.useModal();
  const { t } = useTranslation();
  const currentProviderId = useChatStore((s) => s.selectedProviderId);
  const currentModelId = useChatStore((s) => s.selectedModelId);
  const setSelectedProviderModel = useChatStore(
    (s) => s.setSelectedProviderModel,
  );
  const [forkTarget, setForkTarget] = useState<string | null>(null);
  const setSupportVision = useAttachmentStore((s) => s.setSupportVision);

  const supportVision = (() => {
    const p = provider.providers.find((pr) => pr.id === currentProviderId);
    const model = p?.models.find((m) => m.id === currentModelId);
    // inputModalities 为空视为存量数据，回退 supportVision，避免隐藏原有图片入口
    const modalities = model?.inputModalities;
    if (modalities && modalities.length > 0) {
      return modalities.includes("image");
    }
    return model?.supportVision ?? false;
  })();

  useEffect(() => {
    setSupportVision(supportVision);
  }, [supportVision, setSupportVision]);

  const { handlePaste, handleFileSelect } = useAttachmentIntake();

  useEffect(() => {
    void useProviderStore
      .getState()
      .loadProviders()
      .then(() => useChatStore.getState().loadConversations())
      .catch(() => useChatStore.getState().loadConversations());
  }, []);

  useEffect(() => {
    if (chat.currentConversationId && chat.conversations.length > 0) {
      const conv = chat.conversations.find(
        (c) => c.id === chat.currentConversationId,
      );
      if (conv) {
        const resolved = resolveProviderModel(
          provider.providers,
          conv.providerId,
          conv.modelId,
        );
        setSelectedProviderModel(resolved.providerId, resolved.modelId);
      }
    }
  }, [
    chat.currentConversationId,
    chat.conversations,
    provider.providers,
    setSelectedProviderModel,
  ]);

  useEffect(() => {
    if (!currentProviderId) return;
    const exists = provider.providers.some((p) => p.id === currentProviderId);
    if (!exists) {
      const resolved = resolveProviderModel(
        provider.providers,
        null,
        currentModelId,
      );
      setSelectedProviderModel(resolved.providerId, resolved.modelId);
    }
  }, [
    provider.providers,
    currentProviderId,
    currentModelId,
    setSelectedProviderModel,
  ]);

  const noProviders = provider.providers.length === 0;

  const handleSend = useCallback(
    (content: string) => {
      void chat.sendMessage(content);
    },
    [chat],
  );

  const handleCancel = useCallback(() => {
    void chat.cancelStream();
  }, [chat]);

  const handleRetry = useCallback(
    (messageId: string) => {
      void chat.retryFromError(messageId);
    },
    [chat],
  );

  const clearContextWindow = useCallback(() => {
    chat.resetContextWindow();
  }, [chat]);

  const handleProviderChange = useCallback(
    (newProviderId: string) => {
      const p = provider.providers.find((pr) => pr.id === newProviderId);
      const modelId = p ? resolveDefaultModel(p) : "";
      setSelectedProviderModel(newProviderId, modelId);
      clearContextWindow();
      if (chat.currentConversationId) {
        void chatService
          .updateConversationProviderAndModel(
            chat.currentConversationId,
            newProviderId,
            modelId,
          )
          .catch((e) => log.error("Failed to update provider and model", e));
      }
    },
    [provider.providers, chat, setSelectedProviderModel, clearContextWindow],
  );

  const handleNewConversation = useCallback(() => {
    if (provider.providers.length === 0) {
      modal.warning({
        title: t("chat.view.no_provider_title"),
        content: t("chat.view.no_provider_content"),
      });
      return;
    }
    const pid = currentProviderId || provider.providers[0].id;
    const p = provider.providers.find((pr) => pr.id === pid);
    const modelId = p ? resolveDefaultModel(p) : "";
    void chat.createConversation(pid, modelId);
  }, [provider.providers, currentProviderId, chat, modal, t]);

  useEffect(() => {
    const handler = () => handleNewConversation();
    window.addEventListener("chat-new-conversation", handler);
    return () => window.removeEventListener("chat-new-conversation", handler);
  }, [handleNewConversation]);

  const handleModelChange = useCallback(
    (modelId: string) => {
      setSelectedProviderModel(currentProviderId, modelId);
      if (chat.currentConversationId) {
        void chat.updateModel(chat.currentConversationId, modelId);
      }
    },
    [chat, currentProviderId, setSelectedProviderModel],
  );

  const handleOpenSettings = useCallback(() => {
    orbService
      .openMainWindow()
      .catch((e) => log.warn("Failed to open main window", e));
  }, []);

  const handleErrorAction = useCallback(
    (code: RecoveryActionCode) => {
      switch (code) {
        case "compress_context":
          void chat.manualCompact();
          break;
        case "start_new_chat":
          handleNewConversation();
          break;
        case "open_model_settings":
          handleOpenSettings();
          break;
        default:
          log.warn("Unhandled error recovery action", { code });
          break;
      }
    },
    [chat, handleNewConversation, handleOpenSettings],
  );

  const handleModeChange = useCallback(
    (modeId: string) => {
      chat.setReasoningChoice("inherit");
      const modes = useAgentModeStore.getState().modes;
      const mode = modes.find((m) => m.id === modeId);
      if (mode?.model) {
        for (const p of provider.providers) {
          if (p.models.some((m) => m.id === mode.model)) {
            setSelectedProviderModel(p.id, mode.model);
            clearContextWindow();
            if (chat.currentConversationId) {
              void chatService
                .updateConversationProviderAndModel(
                  chat.currentConversationId,
                  p.id,
                  mode.model,
                )
                .catch((e) =>
                  log.error(
                    "Failed to update provider and model in modeChange",
                    e,
                  ),
                );
            }
            break;
          }
        }
      }
    },
    [provider.providers, chat, setSelectedProviderModel, clearContextWindow],
  );

  const handleEditMessage = useCallback(
    (messageId: string, newContent: string) => {
      void chat.editMessage(messageId, newContent);
    },
    [chat],
  );

  const handleRegenerateMessage = useCallback(
    (messageId: string) => {
      void chat.regenerateMessage(messageId);
    },
    [chat],
  );

  const handleLoadMore = useCallback(() => {
    void chat.loadMoreMessages();
  }, [chat]);

  const handleDeleteMessage = useCallback(
    (id: string) => {
      void (async () => {
        try {
          const subtreeSize = await chatService.getMessageSubtreeSize(id);
          const confirmed = await new Promise<boolean>((resolve) => {
            modal.confirm({
              title: t("chat.message.confirm_delete_subtree", {
                count: subtreeSize,
              }),
              content: t("chat.message.confirm_delete_subtree_desc"),
              okText: t("common.actions.confirm"),
              cancelText: t("common.actions.cancel"),
              okType: "danger",
              onOk: () => resolve(true),
              onCancel: () => resolve(false),
            });
          });
          if (!confirmed) return;
          void chat.deleteMessage(id);
        } catch (e) {
          log.error("deleteMessage failed", e);
        }
      })();
    },
    [chat, modal, t],
  );

  const handleSwitchBranch = useCallback(
    (messageId: string, direction: "prev" | "next") => {
      void (async () => {
        try {
          const siblings = await chatService.getMessageSiblings(messageId);
          const msg = chat.messages.find((m) => m.id === messageId);
          if (!msg) return;
          const targetIdx =
            direction === "next" ? msg.siblingIndex + 1 : msg.siblingIndex - 1;
          const target = siblings[targetIdx];
          if (target) await chat.switchBranch(target.id);
        } catch (e) {
          log.error("switchBranch failed", e);
        }
      })();
    },
    [chat],
  );

  const handleFork = useCallback((messageId: string) => {
    setForkTarget(messageId);
  }, []);

  const confirmFork = useCallback(async () => {
    if (!forkTarget) return;
    const convId = chat.currentConversationId;
    if (!convId) return;
    try {
      const newConvId = await chatService.forkToNewConversation(
        convId,
        forkTarget,
      );
      setForkTarget(null);
      await chat.loadConversations();
      await chat.switchConversation(newConvId);
    } catch (e) {
      log.error("fork failed", e);
      setForkTarget(null);
    }
  }, [forkTarget, chat]);

  return (
    <FeatureErrorBoundary featureName="对话">
      <div className="chat-view chat-view-compact">
        {modalContextHolder}
        <ForkDialog
          open={forkTarget !== null}
          onFork={() => void confirmFork()}
          onCancel={() => setForkTarget(null)}
        />
        <div className="chat-main">
          <div className="orb-chrome-bar">
            <ProviderSelector
              providerId={currentProviderId}
              providers={provider.providers}
              onProviderChange={handleProviderChange}
              variant="chip"
            />
            <ModelSelector
              providerId={currentProviderId}
              modelId={currentModelId}
              onModelChange={handleModelChange}
              providers={provider.providers}
              width={120}
              variant="chip"
              openRef={modelOpenRef}
            />
            <Button
              type="text"
              size="small"
              className="orb-chrome-btn"
              icon={<PlusOutlined />}
              onClick={handleNewConversation}
            />
          </div>
          {chat.messages.length === 0 && noProviders ? (
            <div className="chat-empty-hint">
              <Typography.Text type="secondary">
                {t("chat.view.no_provider_hint")}
              </Typography.Text>
              <Button
                type="primary"
                size="small"
                icon={<SettingOutlined />}
                onClick={handleOpenSettings}
              >
                {t("common.actions.go_settings")}
              </Button>
            </div>
          ) : (
            <MessageList
              messages={chat.messages}
              isStreaming={stream.isStreaming}
              isReasoningStreaming={stream.isReasoningStreaming}
              streamingMessageId={stream.streamingMessageId}
              streamingContent={stream.streamingContent}
              compact
              hasMore={chat.hasMoreMessages}
              isLoadingMore={chat.isLoadingMoreMessages}
              onLoadMore={handleLoadMore}
              onDeleteMessage={handleDeleteMessage}
              onEditMessage={handleEditMessage}
              onRegenerateMessage={handleRegenerateMessage}
              onRetryMessage={handleRetry}
              onSwitchBranch={handleSwitchBranch}
              onFork={handleFork}
              onErrorAction={handleErrorAction}
              retryStatus={stream.retryStatus}
            />
          )}
          <AttachmentPreviewBar />
          <div className="chat-tool-bar">
            <ModeSelector variant="chip" onModeChange={handleModeChange} />
            <ReasoningSelector variant="chip" />
            {!chat.isFull && (
              <Tooltip title={t("chat.compact.generate_summary")}>
                <Button
                  size="small"
                  type="text"
                  icon={<ThunderboltOutlined />}
                  onClick={() => {
                    modal.confirm({
                      title: t("chat.compact.generate_summary"),
                      content: t("chat.compact.generate_confirm"),
                      onOk: () => chat.manualCompact(),
                    });
                  }}
                  disabled={stream.isStreaming}
                />
              </Tooltip>
            )}
            <Button
              size="small"
              type="text"
              icon={<PaperClipOutlined />}
              onClick={() => {
                void handleFileSelect();
              }}
              disabled={stream.isStreaming}
              title={t("chat.input.attach")}
            />
          </div>
          <MessageInput
            onSend={handleSend}
            onCancel={handleCancel}
            onPaste={handlePaste}
            isStreaming={stream.isStreaming}
            disabled={chat.isFull || noProviders}
            isFull={chat.isFull}
          />
          <CompactBanner />
          <CompactErrorBanner />
          <ContextBar usage={chat.contextUsage} />
        </div>
      </div>
    </FeatureErrorBoundary>
  );
};
