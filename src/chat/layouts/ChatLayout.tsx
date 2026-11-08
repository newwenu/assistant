import React from "react";
import { ConversationFrame } from "./ConversationFrame";
import type { ConversationLayoutProps } from "./layoutProps";

/** 对话链路布局：不引用任何工具 / 审批 / 提问组件，也不构造 `toolConfig`。 */
export const ChatLayout: React.FC<ConversationLayoutProps> = (props) => (
  <ConversationFrame {...props} />
);
