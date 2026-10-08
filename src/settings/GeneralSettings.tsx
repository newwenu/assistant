import { useEffect } from "react";
import { Form, Select, Switch, message } from "antd";
import { useSettingsStore } from "@/stores/settingsStore";
import type { AppConf } from "@/stores/settingsStore";
import { dispatchAppConfChanged } from "./settingsDispatcher";
import { SettingsSection } from "./SettingsSection";
import { SettingsPageShell } from "./SettingsPageShell";

export const GeneralSettings: React.FC = () => {
  const appConf = useSettingsStore((s) => s.appConf);
  const loadAppConf = useSettingsStore((s) => s.loadAppConf);
  const [messageApi, contextHolder] = message.useMessage();
  const [form] = Form.useForm();

  useEffect(() => {
    loadAppConf();
  }, [loadAppConf]);

  useEffect(() => {
    if (!appConf) return;
    form.setFieldsValue({
      language: appConf.language,
      launch_at_login: appConf.launch_at_login,
      close_to_tray: appConf.close_to_tray,
      stay_on_top: appConf.stay_on_top,
    });
  }, [appConf, form]);

  const handleSave = async (values: Partial<AppConf>) => {
    if (!appConf) return;
    const newConf: AppConf = { ...appConf, ...values };
    await dispatchAppConfChanged(newConf as unknown as Record<string, unknown>);
    messageApi.success("已保存");
  };

  return (
    <>
      {contextHolder}
      <SettingsPageShell>
        <SettingsSection title="通用设置" style={{ marginTop: 0 }}>
          <Form
            form={form}
            layout="vertical"
            initialValues={{
              language: appConf?.language ?? "zh-CN",
              launch_at_login: appConf?.launch_at_login ?? false,
              close_to_tray: appConf?.close_to_tray ?? true,
              stay_on_top: appConf?.stay_on_top ?? false,
            }}
            onValuesChange={(_changed, all) => handleSave(all)}
          >
            <Form.Item label="语言" name="language">
              <Select
                options={[{ value: "zh-CN", label: "简体中文" }]}
                style={{ width: 200 }}
              />
            </Form.Item>
            <Form.Item
              label="开机自启"
              name="launch_at_login"
              valuePropName="checked"
            >
              <Switch />
            </Form.Item>
            <Form.Item
              label="关闭到托盘"
              name="close_to_tray"
              valuePropName="checked"
              extra="关闭主窗口时最小化到托盘而非退出"
            >
              <Switch />
            </Form.Item>
            <Form.Item
              label="置顶主窗口"
              name="stay_on_top"
              valuePropName="checked"
            >
              <Switch />
            </Form.Item>
          </Form>
        </SettingsSection>
      </SettingsPageShell>
    </>
  );
};
