import React from "react";
import { useTranslation } from "react-i18next";
import type { Message } from "../core/chatTypes";
import { MessageAvatar } from "./MessageAvatar";
import { resolveMessageIdentity } from "./messageIdentity";
import { useAgentModeStore } from "../../stores/agentModeStore";
import { useProviderStore } from "../../stores/providerStore";
import css from "./MessageBubble.module.css";

interface IdentityNodeProps {
  message: Message;
  /** 主会话视图（`MessageList` 的 `compact`）：头像与行高取 18px，与气泡内身份行一致。 */
  compact?: boolean;
}

/**
 * 身份行节点（I46 §5.5）：当同一消息的思考上移为独立节点时，身份行从气泡中拆出、
 * 排在最前，避免「思考节点跑到头像前面」。
 *
 * 复用 `MessageBubble` 的身份行样式与 `resolveMessageIdentity`，只画头像 + 模式 + 模型，
 * 不承载正文 / 错误 / 操作；其余消息的身份行仍留在气泡内，布局零变化。
 * 头像尺寸走同一变量 `--msg-avatar-size`：拆出后不再继承 `.rootRoleAssistant` / `.compact`
 * 的作用域，因此由本节点按 `compact` 重新声明，避免行高与头像跳变。
 */
export const IdentityNode: React.FC<IdentityNodeProps> = ({
  message,
  compact = false,
}) => {
  const { t } = useTranslation();
  const modes = useAgentModeStore((s) => s.modes);
  const providers = useProviderStore((s) => s.providers);
  const identity = resolveMessageIdentity(message, modes, providers);
  const modeText = identity.modeName ?? t("chat.message.identity.unknown_mode");
  const title = [modeText, identity.modelName].filter(Boolean).join(" ");
  const rootClass = [css.identityNode, compact ? css.identityNodeCompact : ""]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={rootClass}>
      <div className={css.identityRow} title={title}>
        <MessageAvatar modeId={message.modeId} />
        <span className={css.identityMode}>{modeText}</span>
        {identity.modelName && (
          <>
            <span className={css.identitySeparator}>
              {t("chat.message.identity.separator")}
            </span>
            <span className={css.identityModel}>{identity.modelName}</span>
          </>
        )}
      </div>
    </div>
  );
};
