import { lazyWithRecovery } from "@/utils/lazyWithRecovery";
import type {
  SettingsPageDefinition,
  SettingsPageManifest,
} from "./types/pageTypes";

export function definePage(
  definition: SettingsPageDefinition,
): SettingsPageManifest {
  const component = lazyWithRecovery(definition.load);
  return {
    ...definition,
    visibility: definition.visibility ?? "sidebar",
    emptyPage: definition.emptyPage ?? false,
    component,
  };
}
