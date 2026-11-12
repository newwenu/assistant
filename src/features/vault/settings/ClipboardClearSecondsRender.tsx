import { useRef, useState } from "react";
import { InputNumber, Modal } from "antd";
import { translateKey } from "@/i18n";
import type { FieldRenderProps } from "@/settings/core/types/fieldSchema";

export function ClipboardClearSecondsRender({
  value,
  onChange,
  onChangeComplete,
  schema,
}: FieldRenderProps) {
  const current = typeof value === "number" ? value : 0;
  const [draft, setDraft] = useState<number | null>(null);
  const valueBeforeEdit = useRef<number>(current);

  const restore = (previous: number) => {
    setDraft(null);
    onChange?.(previous);
  };

  const commit = (next: number | null) => {
    const previous = valueBeforeEdit.current;
    setDraft(null);
    if (next === null || next === previous) {
      if (next === null) restore(previous);
      return;
    }
    if (next === 0) {
      Modal.confirm({
        title: translateKey(
          "settings.vault.clipboard_clear_seconds.confirm_zero_title",
        ),
        content: translateKey(
          "settings.vault.clipboard_clear_seconds.confirm_zero_content",
        ),
        okText: translateKey("common.actions.confirm"),
        cancelText: translateKey("common.actions.cancel"),
        onOk: () => {
          onChangeComplete?.(0);
        },
        onCancel: () => restore(previous),
      });
      return;
    }
    onChangeComplete?.(next);
  };

  return (
    <InputNumber
      min={schema.min}
      max={schema.max}
      step={schema.step}
      value={draft ?? current}
      onFocus={() => {
        valueBeforeEdit.current = current;
      }}
      onChange={(v) => {
        setDraft(typeof v === "number" ? v : null);
        onChange?.(v);
      }}
      onBlur={() => commit(draft)}
      style={{ width: "100%" }}
    />
  );
}
