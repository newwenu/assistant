import React, { useState } from "react";
import { useAgentModeStore } from "../../stores/agent/agentModeStore";
import type { AgentMode } from "../../stores/agent/agentModeStore";
import { BUILTIN_AVATARS, resolveBuiltinAvatar } from "./avatars";
import css from "./MessageAvatar.module.css";

const DATA_IMAGE_PREFIXES = [
  "data:image/png;base64,",
  "data:image/jpeg;base64,",
  "data:image/webp;base64,",
];

/** 内联图长度上限（序列化后 ≤ 8 KB），超出按未知标识兜底（I43 §5.2）。 */
const MAX_DATA_IMAGE_LENGTH = 8 * 1024;

function isAllowedDataImage(value: string): boolean {
  return (
    value.length <= MAX_DATA_IMAGE_LENGTH &&
    DATA_IMAGE_PREFIXES.some((prefix) => value.startsWith(prefix))
  );
}

/** 按 `kind` 推导的兜底头像（降级链第 3 / 4 项，未命中则第 5 项）。 */
function fallbackAvatarFor(mode: AgentMode | undefined): React.FC {
  if (mode?.kind === "agent") return BUILTIN_AVATARS.agent;
  if (mode?.kind === "chat") return BUILTIN_AVATARS.chat;
  return BUILTIN_AVATARS.default;
}

interface ResolvedAvatar {
  imageSrc: string | null;
  Component: React.FC | null;
}

/**
 * 降级链（I43 §5.3）：白名单 `data:` 内联图 → 内置 `builtin:` → 按 `kind` 推导 →
 * 中性兜底。非空但不可用的串（`http(s)` / `asset:` / 非白名单 `data:` / 未知
 * `builtin:`）**直接落第 5 项**，不按 `kind` 推导，且绝不生成 `<img src>`。
 */
function resolveAvatar(
  avatar: string | null,
  mode: AgentMode | undefined,
  imageFailed: boolean,
): ResolvedAvatar {
  if (avatar && !imageFailed) {
    if (isAllowedDataImage(avatar)) {
      return { imageSrc: avatar, Component: null };
    }
    const builtin = resolveBuiltinAvatar(avatar);
    if (builtin) return { imageSrc: null, Component: builtin };
    return { imageSrc: null, Component: BUILTIN_AVATARS.default };
  }
  return { imageSrc: null, Component: fallbackAvatarFor(mode) };
}

interface MessageAvatarProps {
  modeId: string | null;
  /** 显式尺寸；缺省交给 CSS 变量 `--msg-avatar-size`（I43 §5.1 单一来源）。 */
  size?: number;
}

/**
 * 消息身份头像。模式对象直接按 `modeId` 单条订阅，不缓存解析结果：
 * 模式头像编辑后需即时反映（I43 §5.2）。
 */
export const MessageAvatar: React.FC<MessageAvatarProps> = ({
  modeId,
  size,
}) => {
  const mode = useAgentModeStore((s) => s.modes.find((m) => m.id === modeId));
  const [failedSrc, setFailedSrc] = useState<string | null>(null);

  const avatar = mode?.avatar ?? null;
  const imageFailed = avatar !== null && failedSrc === avatar;
  const { imageSrc, Component } = resolveAvatar(avatar, mode, imageFailed);

  return (
    <div
      className={css.root}
      style={size == null ? undefined : { width: size, height: size }}
      aria-hidden="true"
    >
      {imageSrc ? (
        <img
          className={css.image}
          src={imageSrc}
          alt=""
          aria-hidden="true"
          onError={() => setFailedSrc(imageSrc)}
        />
      ) : (
        Component && <Component />
      )}
    </div>
  );
};
