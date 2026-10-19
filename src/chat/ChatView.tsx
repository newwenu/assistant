import React, { useEffect, useState, useCallback, type RefObject } from "react";
import { Button, Modal, Typography, Progress, Tooltip } from "antd";
import {
  PlusOutlined,
  SettingOutlined,
  ThunderboltOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { chatService } from "../services/chatService";
import { orbService } from "../services/orbService";
import { useChatStore } from "../stores/chatStore";
import {
  CONTEXT_WARN_THRESHOLD,
  CONTEXT_DANGER_THRESHOLD,
} from "../stores/chatStore";
import { useProviderStore } from "../stores/providerStore";
import { MessageList } from "./message/MessageList";
import { MessageInput } from "./message/MessageInput";
import { ModelSelector } from "./selectors/ModelSelector";
import { ModeSelector } from "./selectors/ModeSelector";
import { ProviderSelector } from "./selectors/ProviderSelector";
import { CompactBanner } from "./compact/CompactBanner";
import { CompactErrorBanner } from "./compact/CompactErrorBanner";
import { useAgentModeStore } from "../stores/agentModeStore";
import { ForkDialog } from "./blocks/ForkDialog";
import { FeatureErrorBoundary } from "@/components/FeatureErrorBoundary";
import { createLogger } from "@/services/logger";

const log = createLogger("ChatView");

interface ChatViewProps {
  modelOpenRef?: RefObject<boolean>;
}

function ContextBar({
  usage,
}: {
  usage: { usedTokens: number; maxTokens: number; percentage: number };
}) {
  const pct = Math.min(usage.percentage * 100, 100);
  let status: "normal" | "exception" | "active" = "normal";
  if (pct >= CONTEXT_DANGER_THRESHOLD * 100) status = "exception";
  else if (pct >= CONTEXT_WARN_THRESHOLD * 100) status = "active";
  const formatK = (n: number) =>
    n >= 1000 ? `${(n / 1000).toFixed(1)}k` : String(n);
  return (
    <Tooltip
      title={`${Math.round(usage.usedTokens).toLocaleString()} / ${Math.round(usage.maxTokens).toLocaleString()} tokens (${pct.toFixed(1)}%)`}
    >
      <div className="chat-context-bar">
        <span className="chat-context-bar-text">
          {formatK(Math.round(usage.usedTokens))}/
          {formatK(Math.round(usage.maxTokens))}
        </span>
        <Progress
          percent={pct}
          size="small"
          status={status}
          showInfo={false}
          style={{ width: 48, margin: 0 }}
          strokeColor={
            status === "exception"
              ? "#ff4d4f"
              : status === "active"
                ? "#faad14"
                : undefined
          }
        />
        <span className="chat-context-bar-text">{pct.toFixed(0)}%</span>
      </div>
    </Tooltip>
  );
}

export const ChatView: React.FC<ChatViewProps> = ({ modelOpenRef }) => {
  const chat = useChatStore();
  const provider = useProviderStore();
  const [modal, modalContextHolder] = Modal.useModal();
  const { t } = useTranslation();
  const [currentModelId, setCurrentModelId] = useState("");
  const [currentProviderId, setCurrentProviderId] = useState<string | null>(
    null,
  );
  const [forkTarget, setForkTarget] = useState<string | null>(null);

  useEffect(() => {
    void useProviderStore.getState().loadProviders();
    void useChatStore.getState().loadConversations();
  }, []);

  const resolveProviderModel = useCallback(
    (providerId: string | null, modelId: string) => {
      const availableProviders = provider.providers;
      if (availableProviders.length === 0) {
        return { providerId: null, modelId: "" };
      }
      const p = availableProviders.find((pr) => pr.id === providerId);
      if (!p) {
        const fallback = availableProviders[0];
        let fallbackModel = fallback.defaultModel || "";
        if (fallback.models.length > 0 && !fallbackModel)
          fallbackModel = fallback.models[0].id;
        return { providerId: fallback.id, modelId: fallbackModel };
      }
      const models = p.models;
      const modelExists = models.some((m) => m.id === modelId);
      if (!modelExists) {
        let fallbackModel = p.defaultModel || "";
        if (models.length > 0 && !fallbackModel) fallbackModel = models[0].id;
        return { providerId: p.id, modelId: fallbackModel };
      }
      return { providerId: p.id, modelId };
    },
    [provider.providers],
  );

  useEffect(() => {
    if (chat.currentConversationId && chat.conversations.length > 0) {
      const conv = chat.conversations.find(
        (c) => c.id === chat.currentConversationId,
      );
      if (conv) {
        const resolved = resolveProviderModel(conv.providerId, conv.modelId);
        setCurrentProviderId(resolved.providerId);
        setCurrentModelId(resolved.modelId);
      }
    }
  }, [chat.currentConversationId, chat.conversations, resolveProviderModel]);

  useEffect(() => {
    if (!currentProviderId) return;
    const exists = provider.providers.some((p) => p.id === currentProviderId);
    if (!exists) {
      const resolved = resolveProviderModel(null, currentModelId);
      setCurrentProviderId(resolved.providerId);
      setCurrentModelId(resolved.modelId);
    }
  }, [
    provider.providers,
    currentProviderId,
    currentModelId,
    resolveProviderModel,
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

  const handleProviderChange = useCallback(
    (newProviderId: string) => {
      setCurrentProviderId(newProviderId);
      const p = provider.providers.find((pr) => pr.id === newProviderId);
      let modelId = p?.defaultModel || "";
      if (p && p.models.length > 0 && !modelId) modelId = p.models[0].id;
      setCurrentModelId(modelId);
      if (chat.currentConversationId) {
        chatService
          .updateConversationProvider(chat.currentConversationId, newProviderId)
          .catch((e) => log.error("Failed to update provider", e));
        void chat.updateModel(chat.currentConversationId, modelId);
      }
    },
    [provider.providers, chat],
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
    let modelId = p?.defaultModel || "";
    if (p && p.models.length > 0 && !modelId) modelId = p.models[0].id;
    void chat.createConversation(pid, modelId);
  }, [provider.providers, currentProviderId, chat, modal, t]);

  useEffect(() => {
    const handler = () => handleNewConversation();
    window.addEventListener("chat-new-conversation", handler);
    return () => window.removeEventListener("chat-new-conversation", handler);
  }, [handleNewConversation]);

  const handleModelChange = useCallback(
    (modelId: string) => {
      setCurrentModelId(modelId);
      if (chat.currentConversationId) {
        void chat.updateModel(chat.currentConversationId, modelId);
      }
    },
    [chat],
  );

  const handleOpenSettings = useCallback(() => {
    orbService
      .openMainWindow()
      .catch((e) => log.warn("Failed to open main window", e));
  }, []);

  const handleModeChange = useCallback(
    (modeId: string) => {
      const modes = useAgentModeStore.getState().modes;
      const mode = modes.find((m) => m.id === modeId);
      if (mode?.model) {
        for (const p of provider.providers) {
          if (p.models.some((m) => m.id === mode.model)) {
            setCurrentProviderId(p.id);
            setCurrentModelId(mode.model);
            if (chat.currentConversationId) {
              chatService
                .updateConversationProvider(chat.currentConversationId, p.id)
                .catch((e) =>
                  log.warn(
                    "updateConversationProvider failed in modeChange",
                    e,
                  ),
                );
              void chat.updateModel(chat.currentConversationId, mode.model);
            }
            break;
          }
        }
      }
    },
    [provider.providers, chat],
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
              isStreaming={chat.isStreaming}
              isReasoningStreaming={chat.isReasoningStreaming}
              streamingMessageId={chat.streamingMessageId}
              streamingContent={chat.streamingContent}
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
              retryStatus={chat.retryStatus}
            />
          )}
          <div className="chat-tool-bar">
            <ModeSelector variant="chip" onModeChange={handleModeChange} />
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
                  disabled={chat.isStreaming}
                />
              </Tooltip>
            )}
          </div>
          <MessageInput
            onSend={handleSend}
            onCancel={handleCancel}
            isStreaming={chat.isStreaming}
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
