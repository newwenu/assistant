import { useMemo } from "react";
import { Typography, Tag } from "antd";
import type { SettingFieldProps } from "../core/fieldRegistry";
import { ShortcutRecorder } from "./ShortcutRecorder";
import {
  detectConflicts,
  SHORTCUT_DEFINITIONS,
  DEFAULT_KEYBINDINGS,
  normalizeBinding,
  type ConflictInfo,
} from "../core/shortcutTypes";
import { useSettingsStore } from "@/stores/settingsStore";

const { Text } = Typography;

function shortcutIdFromKey(key: string): string | undefined {
  if (!key.startsWith("keybindings.")) return undefined;
  return key.slice("keybindings.".length);
}

export function KeybindingField({
  schema,
  value,
  onChange,
}: SettingFieldProps) {
  const shortcutId = shortcutIdFromKey(schema.key);
  const appConf = useSettingsStore((s) => s.appConf);
  const keybindings = appConf?.keybindings ?? DEFAULT_KEYBINDINGS;

  const conflicts = useMemo(() => detectConflicts(keybindings), [keybindings]);

  const conflict: ConflictInfo | undefined = shortcutId
    ? conflicts[shortcutId]
    : undefined;

  const shortcutDef = shortcutId
    ? SHORTCUT_DEFINITIONS.find((s) => s.id === shortcutId)
    : undefined;

  const defaultBinding = shortcutDef?.defaultBinding;
  const isModified =
    defaultBinding !== undefined &&
    normalizeBinding(value as string) !== normalizeBinding(defaultBinding);

  const handleConfirm = (binding: string) => {
    onChange(binding);
  };

  const handleClear = () => {
    onChange("");
  };

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        width: "100%",
        gap: 8,
      }}
    >
      <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
        {conflict && (
          <Text
            type={conflict.level === "error" ? "danger" : "warning"}
            style={{ fontSize: 12 }}
          >
            {conflict.level === "error"
              ? "同组快捷键冲突"
              : "跨组快捷键冲突，可能无法同时生效"}
          </Text>
        )}
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
        <ShortcutRecorder
          currentBinding={(value as string) ?? ""}
          onConfirm={handleConfirm}
          onClear={handleClear}
        />
        {isModified && <Tag color="orange">已修改</Tag>}
      </div>
    </div>
  );
}
