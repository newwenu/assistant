import React, { useEffect, useState, useCallback, type RefObject } from "react";
import { Button, Modal, Typography, Progress, Tooltip } from "antd";
import { PlusOutlined, SettingOutlined } from "@ant-design/icons";
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
import { ProviderSelector } from "./ProviderSelector";

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
  const [currentModelId, setCurrentModelId] = useState("");
  const [currentProviderId, setCurrentProviderId] = useState<string | null>(
    null,
  );

  useEffect(() => {
    provider.loadProviders();
    chat.loadConversations();
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
          const models = JSON.parse(fallback.models || "[]");
          if (models.length > 0 && !fallbackModel) fallbackModel = models[0].id;
        } catch {
          /* ignore */
        }
        return { providerId: fallback.id, modelId: fallbackModel };
      }
      let models: { id: string }[] = [];
      try {
        models = JSON.parse(p.models || "[]");
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
      chat.sendMessage(content);
    },
    [chat],
  );

  const handleCancel = useCallback(() => {
    chat.cancelStream();
  }, [chat]);

  const handleRetry = useCallback(() => {
    const lastUserMsg = [...chat.messages]
      .reverse()
      .find((m) => m.role === "user");
    if (lastUserMsg) {
      chat.sendMessage(lastUserMsg.content);
    }
  }, [chat]);

  const handleProviderChange = useCallback(
    (newProviderId: string) => {
      setCurrentProviderId(newProviderId);
      const p = provider.providers.find((pr) => pr.id === newProviderId);
      let modelId = p?.default_model || "";
      if (p) {
        try {
          const models = JSON.parse(p.models || "[]");
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
        chat.updateModel(chat.currentConversationId, modelId);
      }
    },
    [provider.providers, chat],
  );

  const handleNewConversation = useCallback(() => {
    if (provider.providers.length === 0) {
      Modal.warning({
        title: "请先配置 AI Provider",
        content: "在设置中添加 AI Provider 后即可开始对话",
      });
      return;
    }
    const pid = currentProviderId || provider.providers[0].id;
    const p = provider.providers.find((pr) => pr.id === pid);
    let modelId = p?.default_model || "";
    if (p) {
      try {
        const models = JSON.parse(p.models || "[]");
        if (models.length > 0 && !modelId) modelId = models[0].id;
      } catch {
        // ignore
      }
    }
    chat.createConversation(pid, modelId);
  }, [provider.providers, currentProviderId, chat]);

  useEffect(() => {
    const handler = () => handleNewConversation();
    window.addEventListener("chat-new-conversation", handler);
    return () => window.removeEventListener("chat-new-conversation", handler);
  }, [handleNewConversation]);

  const handleModelChange = useCallback(
    (modelId: string) => {
      setCurrentModelId(modelId);
      if (chat.currentConversationId) {
        chat.updateModel(chat.currentConversationId, modelId);
      }
    },
    [chat],
  );

  const handleOpenSettings = useCallback(() => {
    invoke("open_main_window");
  }, []);

  const handleEditMessage = useCallback(
    (messageId: string, newContent: string) => {
      chat.editMessage(messageId, newContent);
    },
    [chat],
  );

  const handleRegenerateMessage = useCallback(
    (messageId: string) => {
      chat.regenerateMessage(messageId);
    },
    [chat],
  );

  return (
    <div className="chat-view chat-view-compact">
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
              配置 AI Provider 后即可开始对话
            </Typography.Text>
            <Button
              type="primary"
              size="small"
              icon={<SettingOutlined />}
              onClick={handleOpenSettings}
            >
              去设置
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
            onLoadMore={chat.loadMoreMessages}
            onDeleteMessage={chat.deleteMessage}
            onEditMessage={handleEditMessage}
            onRegenerateMessage={handleRegenerateMessage}
            onRetryMessage={handleRetry}
          />
        )}
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
