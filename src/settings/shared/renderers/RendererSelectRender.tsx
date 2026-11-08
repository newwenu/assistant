import { Select, Typography } from "antd";
import { translateKey } from "@/i18n";
import type { FieldRenderProps } from "../../core/types/fieldSchema";
import { getAllRendererMetas, asRendererId } from "@/mascot";

const { Text } = Typography;

export function RendererSelectRender({ value, onChange }: FieldRenderProps) {
  const metas = getAllRendererMetas();
  const currentMeta = metas.find(
    (m) => m.id === asRendererId((value as string) ?? "svg"),
  );

  return (
    <>
      <Select
        value={value ?? "svg"}
        onChange={(v) => onChange?.(v)}
        style={{ width: "100%" }}
        options={metas.map((m) => ({
          value: m.id,
          label: translateKey(m.label),
        }))}
      />
      {currentMeta?.limitations && (
        <Text type="warning" style={{ display: "block", marginTop: 4 }}>
          {translateKey(currentMeta.limitations)}
        </Text>
      )}
    </>
  );
}
