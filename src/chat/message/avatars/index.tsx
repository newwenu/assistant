import React from "react";

/** 内置头像值前缀；库内 `agent_modes.avatar` 用 `builtin:<name>` 引用。 */
export const BUILTIN_AVATAR_PREFIX = "builtin:";

export type BuiltinAvatarName = "chat" | "agent" | "translate" | "default";

const svgProps = {
  viewBox: "0 0 24 24",
  width: "100%",
  height: "100%",
  "aria-hidden": "true",
} as const;

const ChatAvatar: React.FC = () => (
  <svg {...svgProps}>
    <rect
      x="2.5"
      y="4.5"
      width="19"
      height="13"
      rx="4"
      fill="#e6f0ff"
      stroke="#3b82f6"
      strokeWidth="1.5"
    />
    <path d="M8 17.5l-1.5 3.5 4.5-3.5" fill="#3b82f6" />
    <circle cx="8.5" cy="11" r="1.4" fill="#3b82f6" />
    <circle cx="12" cy="11" r="1.4" fill="#3b82f6" />
    <circle cx="15.5" cy="11" r="1.4" fill="#3b82f6" />
  </svg>
);

const AgentAvatar: React.FC = () => (
  <svg {...svgProps}>
    <rect x="3" y="6" width="18" height="13" rx="4" fill="#f1e9ff" />
    <rect
      x="3"
      y="6"
      width="18"
      height="13"
      rx="4"
      fill="none"
      stroke="#8b5cf6"
      strokeWidth="1.5"
    />
    <circle cx="8.5" cy="12" r="1.6" fill="#8b5cf6" />
    <circle cx="15.5" cy="12" r="1.6" fill="#8b5cf6" />
    <path d="M12 6V3.5" stroke="#8b5cf6" strokeWidth="1.5" />
    <circle cx="12" cy="3" r="1.6" fill="#8b5cf6" />
    <path
      d="M9 16h6"
      stroke="#8b5cf6"
      strokeWidth="1.5"
      strokeLinecap="round"
    />
  </svg>
);

const TranslateAvatar: React.FC = () => (
  <svg {...svgProps}>
    <circle
      cx="12"
      cy="12"
      r="9"
      fill="#e6fbfb"
      stroke="#06b6d4"
      strokeWidth="1.5"
    />
    <path
      d="M6.5 10h7M13.5 10l-2-2M13.5 10l-2 2"
      stroke="#06b6d4"
      strokeWidth="1.5"
      strokeLinecap="round"
    />
    <path
      d="M17.5 15h-7M10.5 15l2-2M10.5 15l2 2"
      stroke="#06b6d4"
      strokeWidth="1.5"
      strokeLinecap="round"
    />
  </svg>
);

const DefaultAvatar: React.FC = () => (
  <svg {...svgProps}>
    <circle
      cx="12"
      cy="12"
      r="9"
      fill="#eff3f8"
      stroke="#94a3b8"
      strokeWidth="1.5"
    />
    <circle cx="12" cy="9.5" r="3" fill="#94a3b8" />
    <path
      d="M6.8 18.2a5.6 5.6 0 0110.4 0"
      fill="none"
      stroke="#94a3b8"
      strokeWidth="1.5"
      strokeLinecap="round"
    />
  </svg>
);

export const BUILTIN_AVATARS: Record<BuiltinAvatarName, React.FC> = {
  chat: ChatAvatar,
  agent: AgentAvatar,
  translate: TranslateAvatar,
  default: DefaultAvatar,
};

export interface BuiltinAvatarOption {
  /** 落库值（`builtin:<name>`）；设置页「未设置」项由调用方单独追加 `""`。 */
  value: string;
  /** i18n key，选项显示名。 */
  labelKey: string;
}

export const BUILTIN_AVATAR_OPTIONS: BuiltinAvatarOption[] = [
  {
    value: `${BUILTIN_AVATAR_PREFIX}chat`,
    labelKey: "chat.message.identity.avatar.builtin_chat",
  },
  {
    value: `${BUILTIN_AVATAR_PREFIX}agent`,
    labelKey: "chat.message.identity.avatar.builtin_agent",
  },
  {
    value: `${BUILTIN_AVATAR_PREFIX}translate`,
    labelKey: "chat.message.identity.avatar.builtin_translate",
  },
  {
    value: `${BUILTIN_AVATAR_PREFIX}default`,
    labelKey: "chat.message.identity.avatar.builtin_default",
  },
];

/** `builtin:<name>` → 内置头像组件；未知名称或前缀不符返回 `null`。 */
export function resolveBuiltinAvatar(
  value: string | null | undefined,
): React.FC | null {
  if (!value || !value.startsWith(BUILTIN_AVATAR_PREFIX)) return null;
  const name = value.slice(BUILTIN_AVATAR_PREFIX.length);
  return (
    (BUILTIN_AVATARS as Record<string, React.FC | undefined>)[name] ?? null
  );
}
