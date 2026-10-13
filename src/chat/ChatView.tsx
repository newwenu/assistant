import React, { useEffect, useState, useCallback, type RefObject } from "react";
import { Button, Modal, Typography, Progress, Tooltip } from "antd";
import { PlusOutlined, SettingOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { useChatStore } from "../stores/chatStore";
import {
  CONTEXT_WARN_THRESHOLD,
  CONTEXT_DANGER_THRESHOLD,
} from "../stores/chatStore";
import { useProviderStore } from "../stores/providerStore";
import { MessageList } from "./message/MessageList";
import { MessageInput } from "./message/MessageInput";
import { ModelSelector } from "./ModelSelector";
import { ModeSelector } from "./ModeSelector";
import { ProviderSelector } from "./ProviderSelector";
import { useAgentModeStore } from "../stores/agentModeStore";

interface ChatViewProps {
  modelOpenRef?: RefObject<boolean>;
}

function ContextBar({
  usage,
}: {
  usage: { used_tokens: number; max_tokens: number; percentage: number };
}) {
  const pct = Math.min(usage.percentage * 100, 100);
  let status: "normal" | "exception" | "active" = "normal";
  if (pct >= CONTEXT_DANGER_THRESHOLD * 100) status = "exception";
  else if (pct >= CONTEXT_WARN_THRESHOLD * 100) status = "active";
  const formatK = (n: number) =>
    n >= 1000 ? `${(n / 1000).toFixed(1)}k` : String(n);
  return (
    <Tooltip
      title={`${Math.round(usage.used_tokens).toLocaleString()} / ${Math.round(usage.max_tokens).toLocaleString()} tokens (${pct.toFixed(1)}%)`}
    >
      <div className="chat-context-bar">
        <span className="chat-context-bar-text">
          {formatK(Math.round(usage.used_tokens))}/
          {formatK(Math.round(usage.max_tokens))}
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
        let fallbackModel = fallback.default_model || "";
        try {
          const models = JSON.parse(fallback.models || "[]") as {
            id: string;
          }[];
          if (models.length > 0 && !fallbackModel) fallbackModel = models[0].id;
        } catch {
          /* ignore */
        }
        return { providerId: fallback.id, modelId: fallbackModel };
      }
      let models: { id: string }[] = [];
      try {
        models = JSON.parse(p.models || "[]") as { id: string }[];
      } catch {
        /* ignore */
      }
      const modelExists = models.some((m) => m.id === modelId);
      if (!modelExists) {
        let fallbackModel = p.default_model || "";
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
        const resolved = resolveProviderModel(conv.provider_id, conv.model_id);
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

  const handleRetry = useCallback(() => {
    const lastUserMsg = [...chat.messages]
      .reverse()
      .find((m) => m.role === "user");
    if (lastUserMsg) {
      void chat.sendMessage(lastUserMsg.content);
    }
  }, [chat]);

  const handleProviderChange = useCallback(
    (newProviderId: string) => {
      setCurrentProviderId(newProviderId);
      const p = provider.providers.find((pr) => pr.id === newProviderId);
      let modelId = p?.default_model || "";
      if (p) {
        try {
          const models = JSON.parse(p.models || "[]") as { id: string }[];
          if (models.length > 0 && !modelId) modelId = models[0].id;
        } catch {
          // ignore
        }
      }
      setCurrentModelId(modelId);
      if (chat.currentConversationId) {
        invoke("update_conversation_provider", {
          id: chat.currentConversationId,
          providerId: newProviderId,
        }).catch((e) => console.error("Failed to update provider:", e));
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
    let modelId = p?.default_model || "";
    if (p) {
      try {
        const models = JSON.parse(p.models || "[]") as { id: string }[];
        if (models.length > 0 && !modelId) modelId = models[0].id;
      } catch {
        // ignore
      }
    }
    void chat.createConversation(pid, modelId);
  }, [provider.providers, currentProviderId, chat, modal]);

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
    invoke("open_main_window").catch((e) =>
      console.error("Failed to open main window:", e),
    );
  }, []);

  const handleModeChange = useCallback(
    (modeId: string) => {
      const modes = useAgentModeStore.getState().modes;
      const mode = modes.find((m) => m.id === modeId);
      if (mode?.model) {
        for (const p of provider.providers) {
          try {
            const models = JSON.parse(p.models || "[]") as { id: string }[];
            if (models.some((m) => m.id === mode.model)) {
              setCurrentProviderId(p.id);
              setCurrentModelId(mode.model);
              if (chat.currentConversationId) {
                invoke("update_conversation_provider", {
                  id: chat.currentConversationId,
                  providerId: p.id,
                }).catch(() => {});
                void chat.updateModel(chat.currentConversationId, mode.model);
              }
              break;
            }
          } catch {
            /* ignore */
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

  return (
    <div className="chat-view chat-view-compact">
      {modalContextHolder}
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
            onLoadMore={() => {
              void chat.loadMoreMessages();
            }}
            onDeleteMessage={(id) => {
              void chat.deleteMessage(id);
            }}
            onEditMessage={handleEditMessage}
            onRegenerateMessage={handleRegenerateMessage}
            onRetryMessage={handleRetry}
          />
        )}
        <div className="orb-mode-bar">
          <ModeSelector variant="chip" onModeChange={handleModeChange} />
        </div>
        <MessageInput
          onSend={handleSend}
          onCancel={handleCancel}
          isStreaming={chat.isStreaming}
          disabled={false}
        />
        <ContextBar usage={chat.contextUsage} />
      </div>
    </div>
  );
};
