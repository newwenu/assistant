import React, {
  useEffect,
  useState,
  useCallback,
  useMemo,
  useRef,
  type RefObject,
} from "react";
import { Modal } from "antd";
import { useTranslation } from "react-i18next";
import { chatService } from "../services/chatService";
import { openFromPanel } from "@/orb/panelActions";
import { useChatStore } from "../stores/chatStore";
import { useChatStreamStore } from "../stores/chatStreamStore";
import { useProviderStore } from "../stores/providerStore";
import { useAgentModeStore } from "../stores/agentModeStore";
import { useAttachmentStore } from "../stores/attachmentStore";
import {
  resolveDefaultModel,
  resolveProviderModel,
} from "./core/providerModel";
import { resolveConversationRoute } from "./core/conversationLink";
import {
  DEFAULT_TOOL_CONFIG,
  diffToolConfig,
  isEmptyToolConfigDiff,
  resolveEffectiveToolConfig,
} from "./core/toolConfig";
import type { ConversationToolConfig } from "./core/chatTypes";
import { notifyInfo, notifyError } from "../stores/notifyStore";
import type { RecoveryActionCode } from "./core/chatErrorRecovery";
import { AgentLayout } from "./layouts/AgentLayout";
import { ChatLayout } from "./layouts/ChatLayout";
import { ModeSelector } from "./selectors/ModeSelector";
import { WorkspaceSelector } from "./selectors/WorkspaceSelector";
import { ToolConfigPanel } from "./tool/ToolConfigPanel";
import { ToolToolbar } from "./tool/ToolToolbar";
import { ToolChangeNotice } from "./tool/ToolChangeNotice";
import { ForkDialog } from "./blocks/ForkDialog";
import { FeatureErrorBoundary } from "@/components/FeatureErrorBoundary";
import { createLogger } from "@/services/logger";
import type { ToolDescriptorDto } from "../types/protocol.contract";
import { useAttachmentIntake } from "./attachment";
import { prefetchAttachmentLimits } from "./attachment/limits";

const log = createLogger("ConversationView");

interface ConversationViewProps {
  modelOpenRef?: RefObject<boolean>;
}

/**
 * chat 域的唯一视图入口：持有会话级状态与命令、模式切换、布局选择（I38d §4.10）。
 * 布局只由 `resolveConversationRoute(...).link` 决定，不按消息内容或全局选中态判定。
 */
export const ConversationView: React.FC<ConversationViewProps> = ({
  modelOpenRef,
}) => {
  const chat = useChatStore();
  const stream = useChatStreamStore();
  const provider = useProviderStore();
  const modes = useAgentModeStore((s) => s.modes);
  const loadModes = useAgentModeStore((s) => s.loadModes);
  const [modal, modalContextHolder] = Modal.useModal();
  const { t } = useTranslation();
  const currentProviderId = useChatStore((s) => s.selectedProviderId);
  const currentModelId = useChatStore((s) => s.selectedModelId);
  const setSelectedProviderModel = useChatStore(
    (s) => s.setSelectedProviderModel,
  );
  const [forkTarget, setForkTarget] = useState<string | null>(null);
  const setVisionSupport = useAttachmentStore((s) => s.setVisionSupport);

  const conversationId = chat.currentConversationId;
  const conversation = chat.conversations.find((c) => c.id === conversationId);

  /** 输入草稿按 `conversationId` 存内存，布局替换不丢草稿（I38d §4.10）。 */
  const draftsRef = useRef<Map<string, string>>(new Map());
  const [draft, setDraft] = useState("");
  const draftKey = conversationId ?? "";
  useEffect(() => {
    setDraft(draftsRef.current.get(draftKey) ?? "");
  }, [draftKey]);
  const handleDraftChange = useCallback(
    (value: string) => {
      draftsRef.current.set(draftKey, value);
      setDraft(value);
    },
    [draftKey],
  );

  const modeId = conversation?.modeId ?? null;
  const route = resolveConversationRoute(
    modeId,
    modes,
    conversation?.toolConfig ?? null,
  );

  const visionSupport = (() => {
    const p = provider.providers.find((pr) => pr.id === currentProviderId);
    const model = p?.models.find((m) => m.id === currentModelId);
    // 模型未找到：保持 unsupported（未选模型时不冒出图片入口）
    if (!model) return "unsupported" as const;
    // 三态事实优先；null 表示存量 models_json，按旧字段派生（与后端同一口径）
    if (model.vision) return model.vision.support;
    const modalities = model.inputModalities;
    if (modalities && modalities.length > 0) {
      return modalities.includes("image")
        ? ("supported" as const)
        : ("unsupported" as const);
    }
    return model.supportVision
      ? ("supported" as const)
      : ("unsupported" as const);
  })();

  useEffect(() => {
    setVisionSupport(visionSupport);
  }, [visionSupport, setVisionSupport]);

  const { handlePaste, handleFileSelect } = useAttachmentIntake();

  useEffect(() => {
    void prefetchAttachmentLimits().catch(() => {});
    void useProviderStore
      .getState()
      .loadProviders()
      .then(() => useChatStore.getState().loadConversations())
      .catch(() => useChatStore.getState().loadConversations());
  }, []);

  useEffect(() => {
    if (modes.length === 0) void loadModes();
  }, [modes.length, loadModes]);

  useEffect(() => {
    if (conversationId && chat.conversations.length > 0) {
      const conv = chat.conversations.find((c) => c.id === conversationId);
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
    conversationId,
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

  /** 播种按会话只做一次：写入失败回滚为 `null` 后不再自动重试，避免死循环（I38d §4.4）。 */
  const seededConversationRef = useRef<Set<string>>(new Set());

  /**
   * 首次进入 agent 布局且会话级配置为 `null` 时播种并落库（I38d §4.4）：
   * 模式级初始值 → 会话级持久值，面板首帧即可显示有效值。
   */
  useEffect(() => {
    if (!conversationId || route.link !== "agent") return;
    if (!conversation || conversation.toolConfig) return;
    if (seededConversationRef.current.has(conversationId)) return;
    seededConversationRef.current.add(conversationId);
    const mode = modes.find((m) => m.id === conversation.modeId);
    void chat.setConversationToolConfig(
      conversationId,
      resolveEffectiveToolConfig(null, mode?.toolConfig),
    );
  }, [conversationId, route.link, conversation, modes, chat]);

  const noProviders = provider.providers.length === 0;
  const isBusy = stream.isStreaming;

  /** 工具清单：agent 链路首次渲染时拉一次，进程内缓存（注册表在启动后不再变化）。 */
  const [tools, setTools] = useState<ToolDescriptorDto[]>([]);
  const [toolsLoading, setToolsLoading] = useState(false);
  const toolsLoadedRef = useRef(false);
  const [toolPanelOpen, setToolPanelOpen] = useState(false);
  const [settingDefault, setSettingDefault] = useState(false);

  useEffect(() => {
    if (route.link !== "agent" || toolsLoadedRef.current) return;
    toolsLoadedRef.current = true;
    setToolsLoading(true);
    void chatService
      .listAvailableTools()
      .then(setTools)
      .catch((e) => {
        log.error("listAvailableTools failed", e);
        toolsLoadedRef.current = false;
      })
      .finally(() => setToolsLoading(false));
  }, [route.link]);

  /** 切会话即收起面板：面板内容属于当前会话的工具配置。 */
  useEffect(() => {
    setToolPanelOpen(false);
  }, [conversationId]);

  const currentMode = useMemo(
    () => modes.find((m) => m.id === conversation?.modeId) ?? null,
    [modes, conversation?.modeId],
  );

  /**
   * 面板 / 概览条展示用的有效配置：`route.toolConfig` 只在 agent 链路有值，
   * 缺失时回退 `DEFAULT_TOOL_CONFIG`，保证组件拿到完整结构而不是 `null`。
   */
  const effectiveToolConfig = route.toolConfig ?? DEFAULT_TOOL_CONFIG;

  /** 装配变更的系统提示：只展示当前一条，定时自清（I38d §4.7）。 */
  const [toolNotice, setToolNotice] = useState<string | null>(null);
  const noticeTimerRef = useRef<number | null>(null);
  const showToolNotice = useCallback((text: string) => {
    setToolNotice(text);
    if (noticeTimerRef.current !== null) {
      window.clearTimeout(noticeTimerRef.current);
    }
    noticeTimerRef.current = window.setTimeout(() => setToolNotice(null), 6000);
  }, []);

  useEffect(
    () => () => {
      if (noticeTimerRef.current !== null) {
        window.clearTimeout(noticeTimerRef.current);
      }
    },
    [],
  );

  useEffect(() => {
    setToolNotice(null);
  }, [conversationId]);

  const handleToolConfigChange = useCallback(
    (next: ConversationToolConfig) => {
      if (!conversationId) return;
      const diff = diffToolConfig(effectiveToolConfig, next);
      if (!isEmptyToolConfigDiff(diff)) {
        const nameOf = (id: string) =>
          tools.find((tool) => tool.id === id)?.name ?? id;
        const parts: string[] = [];
        if (diff.addedTools.length > 0) {
          parts.push(
            t("chat.tool.notice.enabled", {
              names: diff.addedTools.map(nameOf).join(", "),
            }),
          );
        }
        if (diff.removedTools.length > 0) {
          parts.push(
            t("chat.tool.notice.disabled", {
              names: diff.removedTools.map(nameOf).join(", "),
            }),
          );
        }
        if (diff.tierChanged) {
          parts.push(
            t("chat.tool.notice.tier", {
              tier: t(`chat.tool.approval.tier.${diff.tierChanged}`),
            }),
          );
        }
        if (diff.levelChanged) {
          parts.push(
            t("chat.tool.notice.level", {
              level: t(`chat.tool.approval.level.${diff.levelChanged}`),
            }),
          );
        }
        showToolNotice(parts.join(t("chat.tool.notice.separator")));
      }
      void chat.setConversationToolConfig(conversationId, next);
    },
    [conversationId, chat, effectiveToolConfig, tools, t, showToolNotice],
  );

  /** 把当前会话有效配置写回模式初始值，影响之后新建的会话（I38d §4.7）。 */
  const handleSetModeDefault = useCallback(() => {
    const config = route.toolConfig;
    if (!currentMode || !config) return;
    setSettingDefault(true);
    void useAgentModeStore
      .getState()
      .saveMode({ ...currentMode, toolConfig: config })
      .then(() =>
        notifyInfo(
          "chat.tool.panel.set_default_done_title",
          "chat.tool.panel.set_default_done_desc",
        ),
      )
      .catch((e) => {
        log.error("setModeDefault failed", e);
        notifyError(
          "chat.tool.panel.set_default_failed_title",
          "chat.tool.panel.set_default_failed_desc",
        );
      })
      .finally(() => setSettingDefault(false));
  }, [currentMode, route.toolConfig]);

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
    void chat.createConversation(pid, modelId, "builtin-default");
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
    void openFromPanel({ kind: "main" });
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
    (nextModeId: string) => {
      chat.setReasoningChoice("inherit");
      const convId = chat.currentConversationId;
      if (convId) {
        void chat.setConversationMode(convId, nextModeId);
      }
      const mode = modes.find((m) => m.id === nextModeId);
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
    [
      provider.providers,
      chat,
      modes,
      setSelectedProviderModel,
      clearContextWindow,
    ],
  );

  const handleEditMessage = useCallback(
    (messageId: string, newContent: string) => {
      void chat.editMessage(messageId, newContent);
    },
    [chat],
  );

  /** 工作区切换：`WorkspaceSelector` 已先登记目录簿，这里只写会话绑定（I38h §9.1）。 */
  const handleWorkspaceSwitch = useCallback(
    async (path: string) => {
      if (!conversationId) return false;
      return chat.setConversationWorkspace(conversationId, path);
    },
    [chat, conversationId],
  );

  const handleWorkspaceClear = useCallback(async () => {
    if (!conversationId) return false;
    return chat.setConversationWorkspace(conversationId, "");
  }, [chat, conversationId]);

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

  const handleCompact = useCallback(() => {
    modal.confirm({
      title: t("chat.compact.generate_summary"),
      content: t("chat.compact.generate_confirm"),
      onOk: () => chat.manualCompact(),
    });
  }, [modal, t, chat]);

  const layoutProps = {
    providerId: currentProviderId,
    providers: provider.providers,
    modelId: currentModelId,
    modelOpenRef,
    onProviderChange: handleProviderChange,
    onModelChange: handleModelChange,
    onNewConversation: handleNewConversation,
    onOpenSettings: handleOpenSettings,

    messages: chat.messages,
    isStreaming: stream.isStreaming,
    isReasoningStreaming: stream.isReasoningStreaming,
    streamingMessageId: stream.streamingMessageId,
    streamingContent: stream.streamingContent,
    hasMore: chat.hasMoreMessages,
    isLoadingMore: chat.isLoadingMoreMessages,
    retryStatus: stream.retryStatus,
    onLoadMore: handleLoadMore,
    onDeleteMessage: handleDeleteMessage,
    onEditMessage: handleEditMessage,
    onRegenerateMessage: handleRegenerateMessage,
    onRetryMessage: handleRetry,
    onSwitchBranch: handleSwitchBranch,
    onFork: handleFork,
    onErrorAction: handleErrorAction,

    noProviders,
    isFull: chat.isFull,
    contextUsage: chat.contextUsage,

    modeControls: (
      <ModeSelector
        variant="chip"
        value={modeId ?? "builtin-default"}
        onModeChange={handleModeChange}
        disabled={isBusy}
      />
    ),
    draft,
    onDraftChange: handleDraftChange,
    onSend: handleSend,
    onCancel: handleCancel,
    onPaste: handlePaste,
    onAttach: () => {
      void handleFileSelect();
    },
    onCompact: handleCompact,
  };

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
          {route.link === "agent" ? (
            <AgentLayout
              {...layoutProps}
              workspaceControl={
                <WorkspaceSelector
                  workspaceRoot={chat.workspaceRoot}
                  disabled={isBusy}
                  openRef={modelOpenRef}
                  onSwitch={handleWorkspaceSwitch}
                  onClear={handleWorkspaceClear}
                />
              }
              toolToolbar={
                <ToolToolbar
                  config={effectiveToolConfig}
                  total={tools.length}
                  open={toolPanelOpen}
                  onToggle={() => setToolPanelOpen((prev) => !prev)}
                />
              }
              toolPanel={
                toolPanelOpen ? (
                  <ToolConfigPanel
                    config={effectiveToolConfig}
                    tools={tools}
                    loading={toolsLoading}
                    canSetDefault={currentMode?.kind === "agent"}
                    settingDefault={settingDefault}
                    onChange={handleToolConfigChange}
                    onSetDefault={handleSetModeDefault}
                    onClose={() => setToolPanelOpen(false)}
                  />
                ) : null
              }
              belowMessages={
                toolNotice ? <ToolChangeNotice text={toolNotice} /> : null
              }
            />
          ) : (
            <ChatLayout {...layoutProps} />
          )}
        </div>
      </div>
    </FeatureErrorBoundary>
  );
};
