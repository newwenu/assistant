import { useEffect } from "react";
import { Form, Select, Switch, Typography, message } from "antd";
import { useSettingsStore } from "@/stores/settingsStore";
import type { AppConf } from "@/stores/settingsStore";

export const GeneralSettings: React.FC = () => {
  const appConf = useSettingsStore((s) => s.appConf);
  const loadAppConf = useSettingsStore((s) => s.loadAppConf);
  const saveAppConf = useSettingsStore((s) => s.saveAppConf);
  const [messageApi, contextHolder] = message.useMessage();

  useEffect(() => {
    loadAppConf();
  }, [loadAppConf]);

  const handleSave = async (values: Partial<AppConf>) => {
    if (!appConf) return;
    const newConf: AppConf = { ...appConf, ...values };
    await saveAppConf(newConf);
    if (values.launch_at_login !== undefined) {
      try {
        const { enable, disable } =
          await import("@tauri-apps/plugin-autostart");
        if (values.launch_at_login) {
          await enable();
        } else {
          await disable();
        }
      } catch (e) {
        console.error("[GeneralSettings] autostart failed:", e);
      }
    }
    if (values.stay_on_top !== undefined) {
      try {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        await getCurrentWindow().setAlwaysOnTop(values.stay_on_top);
      } catch (e) {
        console.error("[GeneralSettings] setAlwaysOnTop failed:", e);
      }
    }
    messageApi.success("已保存");
  };

  return (
    <>
      {contextHolder}
      <div style={{ maxWidth: 600, margin: "0 auto", padding: 24 }}>
        <Typography.Title level={5} style={{ marginBottom: 16 }}>
          通用设置
        </Typography.Title>
        <Form
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
      </div>
    </>
  );
};