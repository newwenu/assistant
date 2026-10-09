import { Switch, Input, InputNumber, Slider, Select, ColorPicker } from "antd";
import type { SettingFieldProps } from "./settingRegistry";
import { getSettingTypeRenderer } from "./settingRegistry";

function BooleanField({ value, onChange }: SettingFieldProps) {
  return <Switch checked={!!value} onChange={(checked) => onChange(checked)} />;
}

function StringField({ value, onChange }: SettingFieldProps) {
  return (
    <Input value={value as string} onChange={(e) => onChange(e.target.value)} />
  );
}

function NumberField({ value, onChange }: SettingFieldProps) {
  return <InputNumber value={value as number} onChange={(v) => onChange(v)} />;
}

function SliderField({ def, value, onChange }: SettingFieldProps) {
  return (
    <Slider
      min={def.min}
      max={def.max}
      step={def.step ?? 1}
      marks={def.marks}
      value={value as number}
      onChange={(v) => onChange(v)}
    />
  );
}

function SelectField({ def, value, onChange }: SettingFieldProps) {
  return (
    <Select
      options={def.options}
      value={value as string}
      onChange={(v) => onChange(v)}
      style={{ width: 200 }}
    />
  );
}

function ColorField({ value, onChange }: SettingFieldProps) {
  return (
    <ColorPicker
      size="small"
      value={(value as string) ?? undefined}
      onChange={(color) => {
        try {
          const hex = color.toHexString();
          onChange(hex);
        } catch {
          /* ignore */
        }
      }}
    />
  );
}

const BUILTIN_RENDERERS: Record<string, React.FC<SettingFieldProps>> = {
  boolean: BooleanField,
  string: StringField,
  number: NumberField,
  slider: SliderField,
  select: SelectField,
  color: ColorField,
};

export function SettingField({ def, value, onChange }: SettingFieldProps) {
  if (def.customRender) {
    return <def.customRender value={value} onChange={onChange} def={def} />;
  }

  const customRenderer = getSettingTypeRenderer(def.type);
  if (customRenderer) {
    return (
      <customRenderer.component def={def} value={value} onChange={onChange} />
    );
  }

  const BuiltinComponent = BUILTIN_RENDERERS[def.type];
  if (BuiltinComponent) {
    return <BuiltinComponent def={def} value={value} onChange={onChange} />;
  }

  return (
    <Input value={String(value)} onChange={(e) => onChange(e.target.value)} />
  );
}
