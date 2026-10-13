import { Radio } from "antd";
import { useTranslation } from "react-i18next";
import type { FieldRenderProps } from "../core/fieldSchema";

const THEME_OPTIONS = [
  { labelKey: "settings.appearance.theme.options.system", value: "system" },
  { labelKey: "settings.appearance.theme.options.light", value: "light" },
  { labelKey: "settings.appearance.theme.options.dark", value: "dark" },
];

export function ThemeRadioRender({ value, onChange }: FieldRenderProps) {
  const { t } = useTranslation();
  return (
    <Radio.Group
      value={value ?? "system"}
      onChange={(e) => onChange?.(e.target.value)}
    >
      {THEME_OPTIONS.map((opt) => (
        <Radio.Button key={opt.value} value={opt.value}>
          {t(opt.labelKey)}
        </Radio.Button>
      ))}
    </Radio.Group>
  );
}
