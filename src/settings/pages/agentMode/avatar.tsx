import React from "react";
import { Select } from "antd";
import type { TFunction } from "i18next";

import { translateKey } from "@/i18n";
import {
  BUILTIN_AVATAR_OPTIONS,
  BUILTIN_AVATARS,
  resolveBuiltinAvatar,
} from "@/chat/message/avatars";
import type {
  FieldRenderProps,
  SelectOption,
} from "../../core/types/fieldSchema";

export const AvatarPreview: React.FC<{ avatar: string; kind: string }> = ({
  avatar,
  kind,
}) => {
  const Builtin =
    resolveBuiltinAvatar(avatar) ??
    (kind === "agent"
      ? BUILTIN_AVATARS.agent
      : kind === "chat"
        ? BUILTIN_AVATARS.chat
        : BUILTIN_AVATARS.default);
  return (
    <div
      style={{
        width: 20,
        height: 20,
        flex: "0 0 auto",
        borderRadius: 6,
        overflow: "hidden",
      }}
    >
      <Builtin />
    </div>
  );
};

export const AvatarFieldRender: React.FC<FieldRenderProps> = ({
  value,
  onChange,
  schema,
  disabled,
  fieldContext,
}) => {
  const avatar = typeof value === "string" ? value : "";
  const kind = (fieldContext?.formValues.kind as string | undefined) ?? "chat";
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
      <Select
        options={schema.options}
        value={avatar}
        onChange={(v) => onChange?.(v)}
        disabled={disabled}
        style={{ flex: 1 }}
      />
      <AvatarPreview avatar={avatar} kind={kind} />
    </div>
  );
};

export function buildAvatarOptions(t: TFunction): SelectOption[] {
  return [
    { value: "", label: t("settings.agent_mode.avatar.none") },
    ...BUILTIN_AVATAR_OPTIONS.map((o) => ({
      value: o.value,
      label: translateKey(o.labelKey),
    })),
  ];
}
