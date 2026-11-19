import type { ReactNode, RefObject } from "react";
import type {
  ContextUsage,
  ConversationToolConfig,
  Message,
  RetryNotification,
} from "../core/chatTypes";
import type { RecoveryActionCode } from "../core/chatErrorRecovery";
import type { Provider } from "../../settings/provider/types";
import type { ToolDescriptorDto } from "@/types/protocol.contract";

/** 骨架 props：只承载展示所需数据与回调，骨架自身不持有状态（I48 §4）。 */
export interface ConversationLayoutProps {
  /** 布局标识快照：仅透传给分组投影记录来源，不参与任何渲染分支（I46 门禁 16）。 */
  link: "chat" | "agent";
  /** 当前会话 id；原样传给消息区，供滚动意图与位置记忆识别会话（I48 §4）。 */
  conversationId: string | null;
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

  /** 文档面板当前打开的文件路径；`null` 时工具结果恒走卡片（I46 compact §3.7.4）。 */
  docPanelPath?: string | null;
  /** 文档批注跳转回调；与 `docPanelPath` 同源注入。 */
  onOpenDocument?: (path: string, line: number) => void;
}

/** 会话级工作区选择器的数据面（I48 §4；`link === "agent"` 时由骨架挂载）。 */
export interface WorkspaceControlData {
  /** 空串 = 未配置（与 `WorkspaceSelector.workspaceRoot` 语义一致）。 */
  root: string;
  /** turn 进行中禁用。 */
  disabled: boolean;
  /** 菜单展开态回传，供面板统一关闭 / Escape 拦截。 */
  openRef?: RefObject<boolean>;
  /** 返回 `false` 表示失败，保留原值。 */
  onSwitch: (path: string) => Promise<boolean>;
  /** 清除绑定（写回空串）。 */
  onClear: () => Promise<boolean>;
}

/** 视图交给 `useToolRegionState` 的输入（命令侧）。 */
export interface ToolRegionInput {
  conversationId: string | null;
  config: ConversationToolConfig;
  onChange: (next: ConversationToolConfig) => void;
  /** 写回模式默认值；由视图负责落库与提示，hook 只管理进行态并负责剔除伴随 id。 */
  onSetDefault: (config: ConversationToolConfig) => Promise<void>;
}

/** hook 交给骨架的视图数据（展示侧，无状态）。 */
export interface ToolRegionView {
  open: boolean;
  toolbar: {
    config: ConversationToolConfig;
    /** 已启用的可勾选条目数（伴随项不计，与面板计数同口径）。 */
    enabled: number;
    /** 可勾选条目总数（伴随项不计，它们永远没有独立开关）。 */
    total: number;
    onToggle: () => void;
  };
  panel: {
    config: ConversationToolConfig;
    tools: ToolDescriptorDto[];
    loading: boolean;
    canSetDefault: boolean;
    settingDefault: boolean;
    onChange: (next: ConversationToolConfig) => void;
    onSetDefault: () => void;
  };
  notice: string | null;
}

/** 骨架 props：`chat` 链路无工具区域，`agent` 链路必须给出工作区与工具区域数据（I48 §4）。 */
export type ConversationFrameProps = ConversationLayoutProps &
  (
    | { link: "chat" }
    | {
        link: "agent";
        workspace: WorkspaceControlData;
        toolRegion: ToolRegionView;
      }
  );
