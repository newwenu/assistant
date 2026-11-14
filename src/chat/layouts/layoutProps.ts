import type { ReactNode, RefObject } from "react";
import type {
  ContextUsage,
  Message,
  RetryNotification,
} from "../core/chatTypes";
import type { RecoveryActionCode } from "../core/chatErrorRecovery";
import type { Provider } from "../../settings/provider/types";

/** 两个布局共用的 props：只承载展示所需数据与回调，布局自身不持有状态。 */
export interface ConversationLayoutProps {
  /** 布局标识快照：仅透传给分组投影记录来源，不参与任何渲染分支（I46 门禁 16）。 */
  link: "chat" | "agent";
  providerId: string | null;
  providers: Provider[];
  modelId: string;
  modelOpenRef?: RefObject<boolean>;
  onProviderChange: (providerId: string) => void;
  onModelChange: (modelId: string) => void;
  onNewConversation: () => void;
  onOpenSettings: () => void;

  messages: Message[];
  isStreaming: boolean;
  isReasoningStreaming: boolean;
  streamingMessageId: string | null;
  streamingContent: string;
  hasMore: boolean;
  isLoadingMore: boolean;
  retryStatus: RetryNotification | null;
  onLoadMore: () => void;
  onDeleteMessage: (messageId: string) => void;
  onEditMessage: (messageId: string, content: string) => void;
  onRegenerateMessage: (messageId: string) => void;
  onRetryMessage: (messageId: string) => void;
  onSwitchBranch: (messageId: string, direction: "prev" | "next") => void;
  onFork: (messageId: string) => void;
  onErrorAction: (code: RecoveryActionCode) => void;

  noProviders: boolean;
  isFull: boolean;
  contextUsage: ContextUsage;

  /** 模式选择控件（`ModeSelector`，按 kind 分组列出全部模式），两个布局都有。 */
  modeControls: ReactNode;
  draft: string;
  onDraftChange: (value: string) => void;
  onSend: (content: string) => void;
  onCancel: () => void;
  onPaste: (e: React.ClipboardEvent) => void;
  onAttach: () => void;
  onCompact: () => void;
}

/** `AgentLayout` 专属插槽；`ChatLayout` 不传入，因此无工具 UI。 */
export interface AgentLayoutSlots {
  /** 会话级工作区选择器，位于顶部 chrome bar（I38h §9.2）。 */
  workspaceControl?: ReactNode;
  /** 工具概览条，位于输入区工具栏内。 */
  toolToolbar?: ReactNode;
  /** 工具装配面板，位于输入区工具栏上方。 */
  toolPanel?: ReactNode;
  /** 审批 / 提问卡片等消息区下方插槽。 */
  belowMessages?: ReactNode;
  /** 文档面板当前打开的文件路径；`ChatLayout` 不传即 `null`，工具结果恒走卡片（I46 compact §3.7.4）。 */
  docPanelPath?: string | null;
  /** 文档批注跳转回调；与 `docPanelPath` 同源注入。 */
  onOpenDocument?: (path: string, line: number) => void;
}
