import { Switch, Input, InputNumber, Slider, Select, ColorPicker } from "antd";
import type { SettingFieldProps } from "../core/fieldSchema";
import { getSettingTypeRenderer } from "../core/fieldRegistry";

function BooleanField({ value, onChange, disabled }: SettingFieldProps) {
  return (
    <Switch
      checked={!!value}
      onChange={(checked) => onChange(checked)}
      disabled={disabled}
    />
  );
}

function StringField({
  schema,
  value,
  onChange,
  onChangeComplete,
  disabled,
}: SettingFieldProps) {
  return (
    <Input
      placeholder={schema.placeholder}
      value={value as string}
      onChange={(e) => onChange(e.target.value)}
      onBlur={() => onChangeComplete?.(value)}
      onPressEnter={() => onChangeComplete?.(value)}
      disabled={disabled}
    />
  );
}

function TextAreaField({
  schema,
  value,
  onChange,
  onChangeComplete,
  disabled,
}: SettingFieldProps) {
  return (
    <Input.TextArea
      rows={schema.rows ?? 3}
      placeholder={schema.placeholder}
      value={value as string}
      onChange={(e) => onChange(e.target.value)}
      onBlur={() => onChangeComplete?.(value)}
      disabled={disabled}
    />
  );
}

function NumberField({
  value,
  onChange,
  onChangeComplete,
  schema,
  disabled,
}: SettingFieldProps) {
  return (
    <InputNumber
      min={schema.min}
      max={schema.max}
      step={schema.step}
      placeholder={schema.placeholder}
      value={value as number}
      onChange={(v) => onChange(v)}
      onBlur={() => onChangeComplete?.(value)}
      disabled={disabled}
      style={{ width: "100%" }}
    />
  );
}

function SliderField({
  schema,
  value,
  onChange,
  onChangeComplete,
  disabled,
}: SettingFieldProps) {
  return (
    <Slider
      min={schema.min}
      max={schema.max}
      step={schema.step ?? 1}
      marks={schema.marks}
      value={value as number}
      onChange={(v) => onChange(v)}
      onChangeComplete={
        onChangeComplete ? (v) => onChangeComplete(v) : undefined
      }
      disabled={disabled}
    />
  );
}

function SelectField({ schema, value, onChange, disabled }: SettingFieldProps) {
  return (
    <Select
      options={schema.options}
      value={value as string}
      onChange={(v) => onChange(v)}
      allowClear={schema.allowClear}
      placeholder={schema.placeholder}
      disabled={disabled}
      style={{ width: 200 }}
    />
  );
}

function ColorField({
  value,
  onChange,
  onChangeComplete,
  disabled,
}: SettingFieldProps) {
  return (
    <ColorPicker
      size="small"
      disabled={disabled}
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
  textarea: TextAreaField,
  number: NumberField,
  slider: SliderField,
  select: SelectField,
  color: ColorField,
};

export function SettingField({
  schema,
  value,
  onChange,
  onChangeComplete,
  conf,
  disabled,
}: SettingFieldProps) {
  if (schema.customRender) {
    return (
      <schema.customRender
        value={value}
        onChange={onChange}
        onChangeComplete={onChangeComplete}
        schema={schema}
        conf={conf}
        disabled={disabled}
      />
    );
  }

  const customRenderer = getSettingTypeRenderer(schema.type);
  if (customRenderer) {
    return (
      <customRenderer.component
        schema={schema}
        value={value}
        onChange={onChange}
        onChangeComplete={onChangeComplete}
        disabled={disabled}
      />
    );
  }

  const BuiltinComponent = BUILTIN_RENDERERS[schema.type];
  if (BuiltinComponent) {
    return (
      <BuiltinComponent
        schema={schema}
        value={value}
        onChange={onChange}
        onChangeComplete={onChangeComplete}
        conf={conf}
        disabled={disabled}
      />
    );
  }

  return (
    <Input
      value={String(value)}
      onChange={(e) => onChange(e.target.value)}
      disabled={disabled}
    />
  );
}
