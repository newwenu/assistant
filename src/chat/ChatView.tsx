import React, { useEffect, useState, useCallback, type RefObject } from "react";
import { Button, Modal, Typography } from "antd";
import {
  ArrowLeftOutlined,
  PlusOutlined,
  SettingOutlined,
} from "@ant-design/icons";
import { invoke } from "@tauri-apps/api/core";
import { useChatStore } from "../stores/chatStore";
import { useProviderStore } from "../stores/providerStore";
import { MessageList } from "./MessageList";
import { MessageInput } from "./MessageInput";
import { ModelSelector } from "./ModelSelector";
import { ProviderSelector } from "./ProviderSelector";
import { ConversationSidebar } from "./ConversationSidebar";
import { CHAT_PANEL_HEADER_HEIGHT } from "../config/constants";

interface ChatViewProps {
  compact?: boolean;
  modelOpenRef?: RefObject<boolean>;
}

export const ChatView: React.FC<ChatViewProps> = ({
  compact = false,
  modelOpenRef,
}) => {
  const chat = useChatStore();
  const provider = useProviderStore();
  const [currentModelId, setCurrentModelId] = useState("");
  const [currentProviderId, setCurrentProviderId] = useState<string | null>(
    null,
  );
  const [sidebarOpen, setSidebarOpen] = useState(!compact);

  useEffect(() => {
    provider.loadProviders();
    chat.loadConversations();
  }, []);

  useEffect(() => {
    if (chat.currentConversationId && chat.conversations.length > 0) {
      const conv = chat.conversations.find(
        (c) => c.id === chat.currentConversationId,
      );
      if (conv) {
        setCurrentProviderId(conv.provider_id);
        setCurrentModelId(conv.model_id);
      }
    }
  }, [chat.currentConversationId, chat.conversations]);

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

  const toggleSidebar = useCallback(() => {
    setSidebarOpen((v) => !v);
  }, []);

  if (compact) {
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
              compact
              onDeleteMessage={chat.deleteMessage}
              onRetryMessage={handleRetry}
            />
          )}
          <MessageInput
            onSend={handleSend}
            onCancel={handleCancel}
            isStreaming={chat.isStreaming}
            disabled={false}
          />
        </div>
      </div>
    );
  }

  return (
    <div className="chat-view">
      <ConversationSidebar
        conversations={chat.conversations}
        currentId={chat.currentConversationId}
        providerIds={new Set(provider.providers.map((p) => p.id))}
        onSelect={chat.switchConversation}
        onDelete={chat.deleteConversation}
        onRename={chat.renameConversation}
        onCreate={handleNewConversation}
        onSearch={chat.searchConversations}
        onClearSearch={chat.clearSearch}
        searchResults={chat.searchResults}
        visible={sidebarOpen}
      />
      <div className="chat-main">
        <div
          className="chat-header"
          style={{ height: CHAT_PANEL_HEADER_HEIGHT }}
        >
          <div className="chat-header-left">
            <Button
              type="text"
              size="small"
              icon={<ArrowLeftOutlined />}
              onClick={toggleSidebar}
            />
            <Typography.Text
              strong
              ellipsis
              style={{ maxWidth: 120, cursor: "pointer" }}
            >
              {chat.currentConversationId
                ? chat.conversations.find(
                    (c) => c.id === chat.currentConversationId,
                  )?.title || "新对话"
                : "新对话"}
            </Typography.Text>
          </div>
          <div className="chat-header-right">
            <ProviderSelector
              providerId={currentProviderId}
              providers={provider.providers}
              onProviderChange={handleProviderChange}
            />
            <ModelSelector
              providerId={currentProviderId}
              modelId={currentModelId}
              onModelChange={handleModelChange}
              providers={provider.providers}
            />
            <Button
              type="text"
              size="small"
              icon={<PlusOutlined />}
              onClick={handleNewConversation}
            />
          </div>
        </div>
        {chat.messages.length === 0 && noProviders ? (
          <div
            style={{
              flex: 1,
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              justifyContent: "center",
              gap: 12,
            }}
          >
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
            onDeleteMessage={chat.deleteMessage}
            onRetryMessage={handleRetry}
          />
        )}
        <MessageInput
          onSend={handleSend}
          onCancel={handleCancel}
          isStreaming={chat.isStreaming}
          disabled={false}
        />
      </div>
    </div>
  );
};
