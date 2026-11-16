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
import { chatService } from "../services/chat/chatService";
import { openFromPanel } from "@/orb/panelActions";
import { resolveOverlayContainer } from "@/orb/orbOverlays";
import { useChatStore } from "../stores/chat/chatStore";
import { useChatStreamStore } from "../stores/chat/chatStreamStore";
import { useProviderStore } from "../stores/providerStore";
import { useAgentModeStore } from "../stores/agent/agentModeStore";
import { resolveDefaultModel } from "./core/providerModel";
import { resolveConversationRoute } from "./core/message/conversationLink";
import { DEFAULT_TOOL_CONFIG } from "./core/toolConfig";
import type { ConversationToolConfig } from "./core/chatTypes";
import { notifyInfo, notifyError } from "../stores/system/notifyStore";
import type { RecoveryActionCode } from "./core/chatErrorRecovery";
import { ConversationFrame } from "./layouts/ConversationFrame";
import type {
  ConversationFrameProps,
  ConversationLayoutProps,
} from "./layouts/layoutProps";
import { ModeSelector } from "./selectors/ModeSelector";
import { useToolRegionState } from "./tool/useToolRegionState";
import { ForkDialog } from "./blocks/ForkDialog";
import { FeatureErrorBoundary } from "@/components/FeatureErrorBoundary";
import { createLogger } from "@/services/system/logger";
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
  const ensureConversationToolConfig = useChatStore(
    (s) => s.ensureConversationToolConfig,
  );
  const [forkTarget, setForkTarget] = useState<string | null>(null);

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

  /**
   * 首次进入 agent 布局且会话级配置为 `null` 时按模式初始值播种并落库（I38d §4.4）。
   * 幂等与「只播一次」由 store 保证，视图不再持有去重 ref。
   */
  useEffect(() => {
    if (!conversationId || route.link !== "agent") return;
    void ensureConversationToolConfig(conversationId);
  }, [conversationId, route.link, modes.length, ensureConversationToolConfig]);

  const noProviders = provider.providers.length === 0;
  const isBusy = stream.isStreaming;

  const currentMode = useMemo(
    () => modes.find((m) => m.id === conversation?.modeId) ?? null,
    [modes, conversation?.modeId],
  );

  /**
   * 工具区域展示用的有效配置：`route.toolConfig` 只在 agent 链路有值，
   * 缺失时回退 `DEFAULT_TOOL_CONFIG`，保证组件拿到完整结构而不是 `null`。
   */
  const effectiveToolConfig = route.toolConfig ?? DEFAULT_TOOL_CONFIG;

  /** 工具配置的唯一写入函数：提示文案与进行态由 `useToolRegionState` 负责（I48 §5）。 */
  const handleToolConfigChange = useCallback(
    (next: ConversationToolConfig) => {
      if (!conversationId) return;
      void chat.setConversationToolConfig(conversationId, next);
    },
    [conversationId, chat],
  );

  /** 把当前会话有效配置写回模式初始值，影响之后新建的会话（I38d §4.7）。 */
  const handleSetModeDefault = useCallback(async () => {
    const config = route.toolConfig;
    if (!currentMode || !config) return;
    try {
      await useAgentModeStore
        .getState()
        .saveMode({ ...currentMode, toolConfig: config });
      notifyInfo(
        "chat.tool.panel.set_default_done_title",
        "chat.tool.panel.set_default_done_desc",
      );
    } catch (e) {
      log.error("setModeDefault failed", e);
      notifyError(
        "chat.tool.panel.set_default_failed_title",
        "chat.tool.panel.set_default_failed_desc",
      );
    }
  }, [currentMode, route.toolConfig]);

  /** 工具区域（面板 / 概览条 / 变更提示）状态的唯一持有者（I48 §5）。 */
  const toolRegion = useToolRegionState({
    enabled: route.link === "agent",
    conversationId,
    config: effectiveToolConfig,
    onChange: handleToolConfigChange,
    onSetDefault: handleSetModeDefault,
  });

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
        getContainer: resolveOverlayContainer,
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
              getContainer: resolveOverlayContainer,
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
      getContainer: resolveOverlayContainer,
      onOk: () => chat.manualCompact(),
    });
  }, [modal, t, chat]);

  const baseProps: Omit<ConversationLayoutProps, "link"> = {
    conversationId,
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

  /** 联合类型收窄点：`agent` 链路必须同时给出工作区与工具区域数据（I48 §4）。 */
  const frameProps: ConversationFrameProps =
    route.link === "agent"
      ? {
          ...baseProps,
          link: "agent",
          workspace: {
            root: chat.workspaceRoot,
            disabled: isBusy,
            openRef: modelOpenRef,
            onSwitch: handleWorkspaceSwitch,
            onClear: handleWorkspaceClear,
          },
          toolRegion,
        }
      : { ...baseProps, link: "chat" };

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
          <ConversationFrame {...frameProps} />
        </div>
      </div>
    </FeatureErrorBoundary>
  );
};
