import { Radio } from "antd";
import type { FieldRenderProps } from "../core/fieldSchema";

const THEME_OPTIONS = [
  { label: "跟随系统", value: "system" },
  { label: "亮色", value: "light" },
  { label: "暗色", value: "dark" },
];

export function ThemeRadioRender({ value, onChange }: FieldRenderProps) {
  return (
    <Radio.Group
      value={value ?? "system"}
      onChange={(e) => onChange?.(e.target.value)}
    >
      {THEME_OPTIONS.map((opt) => (
        <Radio.Button key={opt.value} value={opt.value}>
          {opt.label}
        </Radio.Button>
      ))}
    </Radio.Group>
  );
}
