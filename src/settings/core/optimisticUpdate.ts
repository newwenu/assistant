import { useSettingsStore } from "@/stores/settingsStore";
import type { SettingsState } from "@/stores/settingsStore";
import { CONF_KEY_MAP, type ConfGroup } from "./field/fieldRegistry";

export function optimisticConfUpdate(
  group: ConfGroup,
  key: string,
  value: unknown,
): void {
  const confKey = CONF_KEY_MAP[group];
  const store = useSettingsStore.getState();
  const current = store[confKey];
  if (!current) return;
  useSettingsStore.setState({
    [confKey]: { ...current, [key]: value },
  } as Partial<SettingsState>);
}
