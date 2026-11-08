import React from "react";
import { ConversationFrame } from "./ConversationFrame";
import type { AgentLayoutSlots, ConversationLayoutProps } from "./layoutProps";

/** Agent 链路布局：复用共享组件，仅追加工具 / 审批 / 提问插槽。 */
export const AgentLayout: React.FC<
  ConversationLayoutProps & AgentLayoutSlots
> = ({ toolbar, panel, belowMessages, ...props }) => (
  <ConversationFrame
    {...props}
    extraToolbar={toolbar}
    extraPanel={panel}
    belowMessages={belowMessages}
  />
);
