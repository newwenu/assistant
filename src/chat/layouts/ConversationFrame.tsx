import React from "react";
import { Button, Typography, Tooltip } from "antd";
import {
  PlusOutlined,
  SettingOutlined,
  ThunderboltOutlined,
  PaperClipOutlined,
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
import type { ConversationLayoutProps } from "./layoutProps";

interface ConversationFrameProps extends ConversationLayoutProps {
  /** 顶部 chrome bar 的工作区选择器（仅 `AgentLayout` 传入）。 */
  workspaceControl?: React.ReactNode;
  /** 输入区工具栏内追加控件（`AgentLayout` 的 `ToolToolbar`）。 */
  toolToolbar?: React.ReactNode;
  /** 输入区工具栏上方追加面板（`AgentLayout` 的 `ToolConfigPanel`）。 */
  toolPanel?: React.ReactNode;
  /** 消息区下方追加插槽（审批 / 提问卡片）。 */
  belowMessages?: React.ReactNode;
}

const AttachmentPreviewBar: React.FC = () => {
  const attachments = useAttachmentStore((s) => s.attachments);
  const removeAttachment = useAttachmentStore((s) => s.removeAttachment);
  return (
    <AttachmentPreview attachments={attachments} onRemove={removeAttachment} />
  );
};

/** 两个布局共享的 chrome / 消息区 / 输入区 / 上下文栏组合，差异只用插槽表达。 */
export const ConversationFrame: React.FC<ConversationFrameProps> = (props) => {
  const {
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
    workspaceControl,
    toolToolbar,
    toolPanel,
    belowMessages,
  } = props;
  const { t } = useTranslation();

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
        {workspaceControl}
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
        />
      )}
      {belowMessages}
      <AttachmentPreviewBar />
      {toolPanel}
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
        {toolToolbar}
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
};
