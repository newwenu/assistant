export interface ShortcutDefinition {
  id: string;
  group: string;
  label: string;
  defaultBinding: string;
  kind: "global" | "in-app";
}

export const SHORTCUT_GROUPS = [
  { key: "global", label: "settings.shortcuts.group.global" },
  { key: "main", label: "settings.shortcuts.group.main" },
] as const;

export const IS_MAC = /\bMacintosh\b/i.test(navigator.userAgent);
export const PRIMARY_MOD = IS_MAC ? "Meta" : "Control";

export const SHORTCUT_DEFINITIONS: ShortcutDefinition[] = [
  {
    id: "toggle_orb",
    group: "global",
    label: "settings.shortcuts.toggle_orb",
    defaultBinding: `${PRIMARY_MOD}+Shift+O`,
    kind: "global",
  },
  {
    id: "toggle_main",
    group: "global",
    label: "settings.shortcuts.toggle_main",
    defaultBinding: `${PRIMARY_MOD}+Shift+M`,
    kind: "global",
  },
  {
    id: "new_chat",
    group: "global",
    label: "settings.shortcuts.new_chat",
    defaultBinding: `${PRIMARY_MOD}+Shift+N`,
    kind: "global",
  },
  {
    id: "command_palette",
    group: "main",
    label: "settings.shortcuts.command_palette",
    defaultBinding: `${PRIMARY_MOD}+K`,
    kind: "in-app",
  },
];

export const DEFAULT_KEYBINDINGS: Record<string, string> = Object.fromEntries(
  SHORTCUT_DEFINITIONS.map((s) => [s.id, s.defaultBinding]),
);

export function normalizeBinding(binding: string): string {
  return binding.replace(/\bControl\b/gi, "Meta");
}

export function formatBinding(binding: string): string {
  if (!binding) return "settings.shortcuts.not_set";
  if (!IS_MAC) {
    return binding
      .split("+")
      .map((t) => {
        const s = t.trim();
        if (s === "Meta" || s === "Control") return "Ctrl";
        if (s === "Alt") return "Alt";
        if (s === "Shift") return "Shift";
        return s;
      })
      .join(" + ");
  }
  return binding
    .split("+")
    .map((token) => {
      switch (token.trim()) {
        case "Control":
          return "⌃";
        case "Alt":
          return "⌥";
        case "Shift":
          return "⇧";
        case "Meta":
          return "⌘";
        default:
          return token.trim();
      }
    })
    .join(" ");
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
