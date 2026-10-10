import { Form, message } from "antd";
import type { SettingDefinition, ConfGroup } from "../core/settingRegistry";
import { getDefinitionsByGroup } from "../core/settingRegistry";
import { SettingField } from "./SettingField";
import { dispatchConfPatch } from "../core/settingsDispatcher";
import { useSettingsStore } from "@/stores/settingsStore";

interface SettingsGroupProps {
  group: ConfGroup;
  definitions?: SettingDefinition[];
  onPatch?: (patch: Record<string, unknown>) => void;
  onPatchComplete?: (patch: Record<string, unknown>) => void;
}

const CONF_KEY_MAP: Record<ConfGroup, "appConf" | "orbConf" | "mascotConf"> = {
  app: "appConf",
  orb: "orbConf",
  mascot: "mascotConf",
};

const DEFAULT_SAVE_MESSAGE = "已保存";

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

function optimisticUpdate(group: ConfGroup, key: string, value: unknown): void {
  const confKey = CONF_KEY_MAP[group];
  const store = useSettingsStore.getState();
  const current = store[confKey] as Record<string, unknown> | null;
  if (!current) return;
  useSettingsStore.setState({
    [confKey]: { ...current, [key]: value },
  });
}

export const SettingsGroup: React.FC<SettingsGroupProps> = ({
  group,
  definitions,
  onPatch,
  onPatchComplete,
}) => {
  const defs = definitions ?? getDefinitionsByGroup(group);
  const conf = useConfByGroup(group);
  const [messageApi, contextHolder] = message.useMessage();

  if (!conf) return null;

  const visibleDefs = defs.filter((d) => !d.visible || d.visible(conf));

  const showSaveMessage = (def: SettingDefinition) => {
    if (def.saveMessage === false) return;
    const msg = def.saveMessage || DEFAULT_SAVE_MESSAGE;
    messageApi.success(msg);
  };

  const handleChange = (def: SettingDefinition, value: unknown) => {
    const patch = { [def.key]: value };
    if (onPatch) {
      onPatch(patch);
    } else {
      dispatchConfPatch(group, patch);
    }
    showSaveMessage(def);
  };

  const handleSliderChange = (key: string, value: unknown) => {
    optimisticUpdate(group, key, value);
  };

  const handleSliderChangeComplete = (
    def: SettingDefinition,
    value: unknown,
  ) => {
    const patch = { [def.key]: value };
    if (onPatchComplete) {
      onPatchComplete(patch);
    } else {
      dispatchConfPatch(group, patch);
    }
    showSaveMessage(def);
  };

  return (
    <>
      {contextHolder}
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
            onChange={
              d.type === "slider"
                ? (v) => handleSliderChange(d.key, v)
                : (v) => handleChange(d, v)
            }
            onChangeComplete={
              d.type === "slider"
                ? (v) => handleSliderChangeComplete(d, v)
                : undefined
            }
          />
        </Form.Item>
      ))}
    </>
  );
};