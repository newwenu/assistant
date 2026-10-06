import { useEffect } from "react";
import { Form, Radio, Typography, message } from "antd";
import { useSettingsStore } from "@/stores/settingsStore";
import { OrbAppearanceSection } from "./OrbAppearanceSection";
import { dispatchThemeChanged } from "./settingsDispatcher";
import type { AppConf } from "@/stores/settingsStore";

export const AppearanceSettings: React.FC = () => {
  const appConf = useSettingsStore((s) => s.appConf);
  const loadAppConf = useSettingsStore((s) => s.loadAppConf);
  const loadOrbConf = useSettingsStore((s) => s.loadOrbConf);
  const saveAppConf = useSettingsStore((s) => s.saveAppConf);
  const [messageApi, contextHolder] = message.useMessage();

  useEffect(() => {
    loadAppConf();
    loadOrbConf();
  }, [loadAppConf, loadOrbConf]);

  const handleThemeChange = async (theme: string) => {
    if (!appConf) return;
    const newConf: AppConf = { ...appConf, theme };
    await saveAppConf(newConf);
    await dispatchThemeChanged(theme as "system" | "light" | "dark");
    messageApi.success("主题已切换");
  };

  return (
    <>
      {contextHolder}
      <div style={{ maxWidth: 600, margin: "0 auto", padding: 24 }}>
        <Typography.Title level={5} style={{ marginBottom: 16 }}>
          主题
        </Typography.Title>
        <Form layout="vertical">
          <Form.Item
            label="主题模式"
            initialValue={appConf?.theme ?? "system"}
          >
            <Radio.Group
              onChange={(e) => handleThemeChange(e.target.value)}
              value={appConf?.theme ?? "system"}
            >
              <Radio.Button value="system">跟随系统</Radio.Button>
              <Radio.Button value="light">亮色</Radio.Button>
              <Radio.Button value="dark">暗色</Radio.Button>
            </Radio.Group>
          </Form.Item>
        </Form>

        <Typography.Title
          level={5}
          style={{ marginTop: 32, marginBottom: 16 }}
        >
          Orb 外观
        </Typography.Title>
        <OrbAppearanceSection />
      </div>
    </>
  );
};