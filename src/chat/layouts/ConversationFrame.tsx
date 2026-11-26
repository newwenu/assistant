import React from "react";
import { Button, Typography, Tooltip } from "antd";
import {
  PlusOutlined,
  SettingOutlined,
  ThunderboltOutlined,
  PaperClipOutlined,
  ScissorOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { ContextBar } from "../context/ContextBar";
import { MessageList } from "../message/MessageList";
import { MessageInput } from "../message/MessageInput";
import { ProviderSelector } from "../selectors/ProviderSelector";
import { ModelSelector } from "../selectors/ModelSelector";
import { ReasoningSelector } from "../selectors/ReasoningSelector";
import { CompactBanner } from "../compact/CompactBanner";
import { AttachmentPreview } from "../message/AttachmentPreview";
import { useAttachmentStore } from "../../stores/attachmentStore";
import { useScreenshotStore } from "../../stores/screenshotStore";
import {
  notifyScreenshotError,
  screenshotService,
} from "../../screenshot/session";
import type { ConversationFrameProps } from "./layoutProps";
import { WorkspaceSelector } from "../selectors/WorkspaceSelector";
import { StartupReadinessNotice } from "../tool/StartupReadinessNotice";
import {
  ToolRegionNotice,
  ToolRegionPanel,
  ToolRegionToolbar,
} from "./ToolRegion";

const AttachmentPreviewBar: React.FC = () => {
  const attachments = useAttachmentStore((s) => s.attachments);
  const removeAttachment = useAttachmentStore((s) => s.removeAttachment);
  return (
    <AttachmentPreview attachments={attachments} onRemove={removeAttachment} />
  );
};

/** 两个链路共享的 chrome / 消息区 / 输入区 / 上下文栏组合，差异只由 `link` 条件挂载（I48 §3）。 */
export function ConversationFrame(props: ConversationFrameProps) {
  const {
    link,
    conversationId,
    providerId,
    providers,
    modelId,
    modelOpenRef,
    onProviderChange,
    onModelChange,
    onNewConversation,
    onOpenSettings,
    messages,
    isStreaming,
    isReasoningStreaming,
    streamingMessageId,
    streamingContent,
    hasMore,
    isLoadingMore,
    retryStatus,
    onLoadMore,
    onDeleteMessage,
    onEditMessage,
    onRegenerateMessage,
    onRetryMessage,
    onSwitchBranch,
    onFork,
    onErrorAction,
    noProviders,
    isFull,
    contextUsage,
    modeControls,
    draft,
    onDraftChange,
    onSend,
    onCancel,
    onPaste,
    onAttach,
    onCompact,
    docPanelPath = null,
    onOpenDocument,
  } = props;
  const { t } = useTranslation();
  /** 联合类型收窄点：`agent` 链路才具备工作区与工具区域数据。 */
  const agentProps = props.link === "agent" ? props : null;

  /**
   * 工具栏截图入口（I35 F41）：先 `beginPreparing()` 让 orb 窗口立刻进入覆盖层模式、
   * 聊天 UI 在抓屏前消失；成功后由 `screenshot-session` 事件复位 `preparing`。
   */
  const onScreenshot = React.useCallback(() => {
    useScreenshotStore.getState().beginPreparing();
    void screenshotService
      .start()
      .then(async () => {
        /**
         * 忙时 `start` 直接返回 Ok 且不发会话事件（Rust 忽略重复入口），
         * 这里补一次拉取：拿到会话就正常显影，拿不到就复位 `preparing`，
         * 否则球体会一直停在隐藏状态。
         */
        if (useScreenshotStore.getState().session) return;
        try {
          const session = await screenshotService.getSession();
          if (!useScreenshotStore.getState().session) {
            useScreenshotStore.getState().setSession(session);
          }
        } catch {
          useScreenshotStore.getState().endPreparing();
        }
      })
      .catch((error: unknown) => {
        useScreenshotStore.getState().endPreparing();
        notifyScreenshotError(error);
      });
  }, []);

  return (
    <>
      <div className="orb-chrome-bar">
        <ProviderSelector
          providerId={providerId}
          providers={providers}
          onProviderChange={onProviderChange}
          variant="chip"
        />
        <ModelSelector
          providerId={providerId}
          modelId={modelId}
          onModelChange={onModelChange}
          providers={providers}
          width={120}
          variant="chip"
          openRef={modelOpenRef}
        />
        {agentProps ? (
          <WorkspaceSelector
            conversationId={conversationId}
            workspaceRoot={agentProps.workspace.root}
            disabled={agentProps.workspace.disabled}
            openRef={agentProps.workspace.openRef}
            onSwitch={agentProps.workspace.onSwitch}
            onClear={agentProps.workspace.onClear}
          />
        ) : null}
        <Button
          type="text"
          size="small"
          className="orb-chrome-btn"
          icon={<PlusOutlined />}
          onClick={onNewConversation}
        />
      </div>
      {messages.length === 0 && noProviders ? (
        <div className="chat-empty-hint">
          <Typography.Text type="secondary">
            {t("chat.view.no_provider_hint")}
          </Typography.Text>
          <Button
            type="primary"
            size="small"
            icon={<SettingOutlined />}
            onClick={onOpenSettings}
          >
            {t("common.actions.go_settings")}
          </Button>
        </div>
      ) : (
        <MessageList
          messages={messages}
          conversationId={conversationId}
          link={link}
          isStreaming={isStreaming}
          isReasoningStreaming={isReasoningStreaming}
          streamingMessageId={streamingMessageId}
          streamingContent={streamingContent}
          compact
          hasMore={hasMore}
          isLoadingMore={isLoadingMore}
          onLoadMore={onLoadMore}
          onDeleteMessage={onDeleteMessage}
          onEditMessage={onEditMessage}
          onRegenerateMessage={onRegenerateMessage}
          onRetryMessage={onRetryMessage}
          onSwitchBranch={onSwitchBranch}
          onFork={onFork}
          onErrorAction={onErrorAction}
          retryStatus={retryStatus}
          docPanelPath={docPanelPath}
          onOpenDocument={onOpenDocument}
        />
      )}
      {agentProps ? <ToolRegionNotice view={agentProps.toolRegion} /> : null}
      {agentProps ? <StartupReadinessNotice /> : null}
      <AttachmentPreviewBar />
      {agentProps ? <ToolRegionPanel view={agentProps.toolRegion} /> : null}
      <div className="chat-tool-bar">
        {modeControls}
        <ReasoningSelector variant="chip" />
        {!isFull && (
          <Tooltip title={t("chat.compact.generate_summary")}>
            <Button
              size="small"
              type="text"
              icon={<ThunderboltOutlined />}
              onClick={onCompact}
              disabled={isStreaming}
            />
          </Tooltip>
        )}
        <Button
          size="small"
          type="text"
          icon={<PaperClipOutlined />}
          onClick={onAttach}
          disabled={isStreaming}
          title={t("chat.input.attach")}
        />
        <Button
          size="small"
          type="text"
          icon={<ScissorOutlined />}
          onClick={onScreenshot}
          disabled={isStreaming}
          title={t("chat.input.screenshot")}
        />
        {agentProps ? <ToolRegionToolbar view={agentProps.toolRegion} /> : null}
      </div>
      <MessageInput
        value={draft}
        onValueChange={onDraftChange}
        onSend={onSend}
        onCancel={onCancel}
        onPaste={onPaste}
        isStreaming={isStreaming}
        disabled={isFull || noProviders}
        isFull={isFull}
      />
      <CompactBanner />
      <ContextBar usage={contextUsage} />
    </>
  );
}
