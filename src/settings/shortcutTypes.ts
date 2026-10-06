export interface ShortcutDefinition {
  id: string;
  group: string;
  label: string;
  defaultBinding: string;
  kind: "global" | "in-app";
}

export const SHORTCUT_GROUPS = [
  { key: "global", label: "全局快捷键" },
  { key: "main", label: "主窗口" },
] as const;

export const SHORTCUT_DEFINITIONS: ShortcutDefinition[] = [
  {
    id: "toggle_orb",
    group: "global",
    label: "显示/隐藏悬浮球",
    defaultBinding: "Super+Shift+O",
    kind: "global",
  },
  {
    id: "toggle_main",
    group: "global",
    label: "显示/隐藏主窗口",
    defaultBinding: "Super+Shift+M",
    kind: "global",
  },
  {
    id: "new_chat",
    group: "global",
    label: "新建对话",
    defaultBinding: "Super+Shift+N",
    kind: "global",
  },
  {
    id: "command_palette",
    group: "main",
    label: "命令面板",
    defaultBinding: "Super+K",
    kind: "in-app",
  },
];

export const DEFAULT_KEYBINDINGS: Record<string, string> = Object.fromEntries(
  SHORTCUT_DEFINITIONS.map((s) => [s.id, s.defaultBinding]),
);

export function formatBinding(binding: string): string {
  if (!binding) return "未设置";
  return binding
    .replace(/Super/g, "⌘")
    .replace(/Shift/g, "⇧")
    .replace(/Alt/g, "⌥")
    .replace(/Control/g, "⌃")
    .replace(/\+/g, " + ");
}

export interface ConflictInfo {
  conflictedIds: string[];
  level: "error" | "warning";
}

export function detectConflicts(
  keybindings: Record<string, string>,
): Record<string, ConflictInfo> {
  const bindingToIds: Record<string, string[]> = {};
  for (const [id, binding] of Object.entries(keybindings)) {
    if (!binding) continue;
    if (!bindingToIds[binding]) bindingToIds[binding] = [];
    bindingToIds[binding].push(id);
  }

  const idToGroup: Record<string, string> = {};
  for (const def of SHORTCUT_DEFINITIONS) {
    idToGroup[def.id] = def.group;
  }

  const conflicts: Record<string, ConflictInfo> = {};
  for (const [, ids] of Object.entries(bindingToIds)) {
    if (ids.length <= 1) continue;
    for (const id of ids) {
      const otherIds = ids.filter((i) => i !== id);
      const myGroup = idToGroup[id] ?? "";
      const hasSameGroupConflict = otherIds.some(
        (other) => (idToGroup[other] ?? "") === myGroup,
      );
      conflicts[id] = {
        conflictedIds: otherIds,
        level: hasSameGroupConflict ? "error" : "warning",
      };
    }
  }
  return conflicts;
}
