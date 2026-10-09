import { useEffect } from "react";
import { Form, Switch } from "antd";
import { useSettingsStore } from "@/stores/settingsStore";
import type { OrbConf } from "@/stores/settingsStore";
import { dispatchOrbConfChanged } from "./settingsDispatcher";

interface OrbPanelSectionProps {
  onSaved?: () => void;
}

export const OrbPanelSection: React.FC<OrbPanelSectionProps> = ({
  onSaved,
}) => {
  const orbConf = useSettingsStore((s) => s.orbConf);
  const [form] = Form.useForm();

  useEffect(() => {
    if (!orbConf) return;
    form.setFieldsValue({
      always_on_top: orbConf.always_on_top,
    });
  }, [orbConf, form]);

  const handleSave = async (values: Partial<OrbConf>) => {
    if (!orbConf) return;
    const newConf: OrbConf = { ...orbConf, ...values };
    await dispatchOrbConfChanged(newConf as unknown as Record<string, unknown>);
    onSaved?.();
  };

  return (
    <Form
      form={form}
      layout="vertical"
      initialValues={{
        always_on_top: orbConf?.always_on_top ?? true,
      }}
      onValuesChange={(_changed, all) => handleSave(all)}
    >
      <Form.Item label="始终置顶" name="always_on_top" valuePropName="checked">
        <Switch />
      </Form.Item>
    </Form>
  );
};
