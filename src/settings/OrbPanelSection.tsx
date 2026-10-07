import { useEffect } from "react";
import { Form, Switch, message } from "antd";
import { useSettingsStore } from "@/stores/settingsStore";
import type { OrbConf } from "@/stores/settingsStore";
import { dispatchOrbConfChanged } from "./settingsDispatcher";

export const OrbPanelSection: React.FC = () => {
  const orbConf = useSettingsStore((s) => s.orbConf);
  const [messageApi, contextHolder] = message.useMessage();
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
    messageApi.success("已保存");
  };

  return (
    <>
      {contextHolder}
      <Form
        form={form}
        layout="vertical"
        initialValues={{
          always_on_top: orbConf?.always_on_top ?? true,
        }}
        onValuesChange={(_changed, all) => handleSave(all)}
      >
        <Form.Item
          label="始终置顶"
          name="always_on_top"
          valuePropName="checked"
          extra="悬浮窗始终保持在其他窗口之上"
        >
          <Switch />
        </Form.Item>
      </Form>
    </>
  );
};
