import { Select, Typography } from "antd";
import type { SettingFieldCustomProps } from "../core/fieldRegistry";
import { getAllRendererMetas, asRendererId } from "@/mascot";

const { Text } = Typography;

export function RendererSelectRender({
  value,
  onChange,
}: SettingFieldCustomProps) {
  const metas = getAllRendererMetas();
  const currentMeta = metas.find(
    (m) => m.id === asRendererId((value as string) ?? "rive"),
  );

  return (
    <>
      <Select
        value={value ?? "rive"}
        onChange={(v) => onChange?.(v)}
        style={{ width: "100%" }}
        options={metas.map((m) => ({
          value: m.id as string,
          label: m.label,
        }))}
      />
      {currentMeta?.limitations && (
        <Text type="warning" style={{ display: "block", marginTop: 4 }}>
          {currentMeta.limitations}
        </Text>
      )}
    </>
  );
}
