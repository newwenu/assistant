import { Switch, Input, InputNumber, Slider, Select, ColorPicker } from "antd";
import type { SettingFieldProps } from "../core/fieldRegistry";
import { getSettingTypeRenderer } from "../core/fieldRegistry";

function BooleanField({ value, onChange }: SettingFieldProps) {
  return <Switch checked={!!value} onChange={(checked) => onChange(checked)} />;
}

function StringField({ value, onChange, onChangeComplete }: SettingFieldProps) {
  return (
    <Input
      value={value as string}
      onChange={(e) => onChange(e.target.value)}
      onBlur={() => onChangeComplete?.(value)}
      onPressEnter={() => onChangeComplete?.(value)}
    />
  );
}

function NumberField({ value, onChange, onChangeComplete }: SettingFieldProps) {
  return (
    <InputNumber
      value={value as number}
      onChange={(v) => onChange(v)}
      onBlur={() => onChangeComplete?.(value)}
    />
  );
}

function SliderField({
  def,
  value,
  onChange,
  onChangeComplete,
}: SettingFieldProps) {
  return (
    <Slider
      min={def.min}
      max={def.max}
      step={def.step ?? 1}
      marks={def.marks}
      value={value as number}
      onChange={(v) => onChange(v)}
      onChangeComplete={
        onChangeComplete ? (v) => onChangeComplete(v) : undefined
      }
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

function ColorField({ value, onChange, onChangeComplete }: SettingFieldProps) {
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
      onChangeComplete={(color) => {
        if (!onChangeComplete) return;
        try {
          const hex = color.toHexString();
          onChangeComplete(hex);
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

export function SettingField({
  def,
  value,
  onChange,
  onChangeComplete,
  conf,
}: SettingFieldProps) {
  if (def.customRender) {
    return (
      <def.customRender
        value={value}
        onChange={onChange}
        onChangeComplete={onChangeComplete}
        def={def}
        conf={conf}
      />
    );
  }

  const customRenderer = getSettingTypeRenderer(def.type);
  if (customRenderer) {
    return (
      <customRenderer.component
        def={def}
        value={value}
        onChange={onChange}
        onChangeComplete={onChangeComplete}
      />
    );
  }

  const BuiltinComponent = BUILTIN_RENDERERS[def.type];
  if (BuiltinComponent) {
    return (
      <BuiltinComponent
        def={def}
        value={value}
        onChange={onChange}
        onChangeComplete={onChangeComplete}
      />
    );
  }

  return (
    <Input value={String(value)} onChange={(e) => onChange(e.target.value)} />
  );
}
