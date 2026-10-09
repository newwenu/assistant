import { Form } from "antd";
import type { SettingDefinition, ConfGroup } from "./settingRegistry";
import { getDefinitionsByGroup } from "./settingRegistry";
import { SettingField } from "./SettingField";
import { dispatchConfPatch } from "./settingsDispatcher";
import { useSettingsStore } from "@/stores/settingsStore";

interface SettingsGroupProps {
  group: ConfGroup;
  definitions?: SettingDefinition[];
  onPatch?: (patch: Record<string, unknown>) => void;
}

function useConfByGroup(group: ConfGroup): Record<string, unknown> | null {
  const appConf = useSettingsStore((s) => s.appConf);
  const orbConf = useSettingsStore((s) => s.orbConf);
  const mascotConf = useSettingsStore((s) => s.mascotConf);
  if (group === "app")
    return appConf as unknown as Record<string, unknown> | null;
  if (group === "orb")
    return orbConf as unknown as Record<string, unknown> | null;
  if (group === "mascot")
    return mascotConf as unknown as Record<string, unknown> | null;
  return null;
}

export const SettingsGroup: React.FC<SettingsGroupProps> = ({
  group,
  definitions,
  onPatch,
}) => {
  const defs = definitions ?? getDefinitionsByGroup(group);
  const conf = useConfByGroup(group);

  if (!conf) return null;

  const visibleDefs = defs.filter((d) => !d.visible || d.visible(conf));

  const handleChange = (key: string, value: unknown) => {
    const patch = { [key]: value };
    if (onPatch) {
      onPatch(patch);
    } else {
      dispatchConfPatch(group, patch);
    }
  };

  return (
    <>
      {visibleDefs.map((d) => (
        <Form.Item
          key={d.key}
          label={d.label}
          tooltip={d.description}
          extra={d.extra}
          valuePropName={d.type === "boolean" ? "checked" : "value"}
        >
          <SettingField
            def={d}
            value={conf[d.key]}
            onChange={(v) => handleChange(d.key, v)}
          />
        </Form.Item>
      ))}
    </>
  );
};
