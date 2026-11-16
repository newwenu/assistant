import { useSettingsStore } from "@/stores/settingsStore";
import type { ConfGroup, ConfStore } from "../types/confRegistry";

export function optimisticConfUpdate<G extends ConfGroup>(
  group: G,
  key: string,
  value: unknown,
): void {
  const state = useSettingsStore.getState();
  const current = state.confs[group];
  if (!current) return;
  const next: ConfStore = { ...state.confs };
  next[group] = Object.assign({}, current, { [key]: value });
  useSettingsStore.setState({ confs: next });
}
