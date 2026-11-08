import type { FC } from "react";
import type { FieldRenderProps } from "./fieldSchema";
import type { SettingDefinition } from "../field/fieldRegistry";

export type ExtensionSettingDefinition = Omit<
  SettingDefinition,
  "customRender"
> & {
  customRenderLoad?: () => Promise<{ default: FC<FieldRenderProps> }>;
};
