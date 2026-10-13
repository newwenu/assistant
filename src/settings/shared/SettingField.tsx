import { useState } from "react";
import {
  Switch,
  Input,
  InputNumber,
  Slider,
  Select,
  ColorPicker,
  Button,
  Space,
  Typography,
} from "antd";
import type {
  SettingFieldProps,
  FieldContext,
  DynamicString,
  FieldActionCallbacks,
} from "../core/fieldSchema";
import { getSettingTypeRenderer } from "../core/fieldRegistry";

function resolveDynamic(
  value: DynamicString | undefined,
  ctx: FieldContext | undefined,
): string | undefined {
  if (value === undefined) return undefined;
  if (typeof value === "function") {
    if (!ctx) return undefined;
    return value(ctx);
  }
  return value;
}

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
  fieldContext,
}: SettingFieldProps) {
  return (
    <Input
      placeholder={resolveDynamic(schema.placeholder, fieldContext)}
      value={value as string}
      onChange={(e) => onChange(e.target.value)}
      onBlur={() => onChangeComplete?.(value)}
      onPressEnter={() => onChangeComplete?.(value)}
      disabled={disabled}
    />
  );
}

function SecretField({
  schema,
  value,
  onChange,
  onChangeComplete,
  disabled,
  fieldContext,
}: SettingFieldProps) {
  return (
    <Input.Password
      placeholder={resolveDynamic(schema.placeholder, fieldContext)}
      value={value as string}
      onChange={(e) => onChange(e.target.value)}
      onBlur={() => onChangeComplete?.(value)}
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
  fieldContext,
}: SettingFieldProps) {
  return (
    <Input.TextArea
      rows={schema.rows ?? 3}
      placeholder={resolveDynamic(schema.placeholder, fieldContext)}
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
  fieldContext,
}: SettingFieldProps) {
  return (
    <InputNumber
      min={schema.min}
      max={schema.max}
      step={schema.step}
      placeholder={resolveDynamic(schema.placeholder, fieldContext)}
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

function SelectField({
  schema,
  value,
  onChange,
  disabled,
  fieldContext,
}: SettingFieldProps) {
  const [dynamicOptions, setDynamicOptions] = useState<
    { label: string; value: string }[] | null
  >(null);
  const [actionLoading, setActionLoading] = useState<Set<string>>(new Set());

  const effectiveOptions = dynamicOptions ?? schema.options;

  const actionCallbacks: FieldActionCallbacks = {
    setFieldOptions: (opts) => setDynamicOptions(opts),
    setFieldValue: (v) => onChange(v),
  };

  return (
    <Space.Compact style={{ width: "100%" }}>
      <Select
        options={effectiveOptions}
        value={value as string}
        onChange={(v) => onChange(v)}
        allowClear={schema.allowClear}
        placeholder={resolveDynamic(schema.placeholder, fieldContext)}
        disabled={disabled}
        style={{ flex: 1, minWidth: 200 }}
      />
      {schema.actions?.map((action) => (
        <Button
          key={action.key}
          icon={action.icon ? <action.icon /> : undefined}
          loading={actionLoading.has(action.key)}
          onClick={() => {
            const ctx = fieldContext ?? {
              mode: "edit" as const,
              formValues: {},
            };
            setActionLoading((prev) => new Set(prev).add(action.key));
            void Promise.resolve(action.onClick(ctx, actionCallbacks)).finally(
              () => {
                setActionLoading((prev) => {
                  const next = new Set(prev);
                  next.delete(action.key);
                  return next;
                });
              },
            );
          }}
          disabled={disabled}
        >
          {action.label}
        </Button>
      ))}
    </Space.Compact>
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

function ReadonlyField({ schema, value }: SettingFieldProps) {
  const text =
    value == null
      ? "-"
      : typeof value === "string"
        ? value
        : JSON.stringify(value);
  if (schema.copyable) {
    return <Typography.Text copyable={{ text }}>{text}</Typography.Text>;
  }
  return <Typography.Text>{text}</Typography.Text>;
}

const BUILTIN_RENDERERS: Record<string, React.FC<SettingFieldProps>> = {
  boolean: BooleanField,
  string: StringField,
  secret: SecretField,
  textarea: TextAreaField,
  number: NumberField,
  slider: SliderField,
  select: SelectField,
  color: ColorField,
  readonly: ReadonlyField,
};

export function SettingField({
  schema,
  value,
  onChange,
  onChangeComplete,
  conf,
  disabled,
  fieldContext,
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
        fieldContext={fieldContext}
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
        fieldContext={fieldContext}
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
        fieldContext={fieldContext}
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
