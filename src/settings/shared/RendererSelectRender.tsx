import { Select, Typography } from "antd";
import { useTranslation } from "react-i18next";
import type { FieldRenderProps } from "../core/fieldSchema";
import { getAllRendererMetas, asRendererId } from "@/mascot";

const { Text } = Typography;

export function RendererSelectRender({ value, onChange }: FieldRenderProps) {
  const { t } = useTranslation();
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
          value: m.id,
          label: t(m.label),
        }))}
      />
      {currentMeta?.limitations && (
        <Text type="warning" style={{ display: "block", marginTop: 4 }}>
          {t(currentMeta.limitations)}
        </Text>
      )}
    </>
  );
}
