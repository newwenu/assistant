export const STRENGTH_COLORS: Record<number, string> = {
  0: "var(--ant-color-error)",
  1: "var(--ant-color-error)",
  2: "var(--ant-color-warning)",
  3: "var(--ant-color-success)",
  4: "var(--ant-color-success)",
};

export const STRENGTH_FALLBACK_COLOR = "var(--ant-color-text-quaternary)";

export const VAULT_TONES = {
  favorite: "var(--ant-color-warning)",
  leak: "var(--ant-color-error)",
  syncReady: "var(--ant-color-success)",
  syncDirty: "var(--ant-color-warning)",
  syncAhead: "var(--ant-color-warning)",
  syncBehind: "var(--ant-color-info)",
  syncDiverged: "var(--ant-color-error)",
  syncOff: "var(--ant-color-text-quaternary)",
  syncNoRemote: "var(--ant-color-text-tertiary)",
  syncFailed: "var(--ant-color-error)",
} as const;
