import type { ReactNode } from "react";

export type ConflictSide = "local" | "remote";

export interface CompareRow {
  key: string;
  label: string;
  local: ReactNode;
  remote: ReactNode;
  localPresent: boolean;
  remotePresent: boolean;
  changed: boolean;
}

/** 遮蔽字段的显示控制器：上层持有遮蔽值与自动重遮蔽计时器。 */
export interface RevealController {
  revealed: Record<string, string>;
  revealingKey: string | null;
  reveal: (side: ConflictSide, field: string) => void;
}

export function formatTime(millis: number): string {
  return new Date(millis).toLocaleString();
}
