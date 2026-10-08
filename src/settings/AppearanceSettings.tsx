import { useEffect } from "react";
import { Form, Radio, message } from "antd";
import { useSettingsStore } from "@/stores/settingsStore";
import { OrbBallSection } from "./OrbBallSection";
import { OrbPanelSection } from "./OrbPanelSection";
import { SettingsSection } from "./SettingsSection";
import { SettingsPageShell } from "./SettingsPageShell";
import {
  dispatchAppConfChanged,
  dispatchThemeChanged,
} from "./settingsDispatcher";
import type { AppConf } from "@/stores/settingsStore";

export const AppearanceSettings: React.FC = () => {
  const appConf = useSettingsStore((s) => s.appConf);
  const loadAppConf = useSettingsStore((s) => s.loadAppConf);
  const loadOrbConf = useSettingsStore((s) => s.loadOrbConf);
  const [messageApi, contextHolder] = message.useMessage();

  useEffect(() => {
    loadAppConf();
    loadOrbConf();
  }, [loadAppConf, loadOrbConf]);

  const handleThemeChange = async (theme: string) => {
    if (!appConf) return;
    const newConf: AppConf = { ...appConf, theme };
    await dispatchAppConfChanged(newConf as unknown as Record<string, unknown>);
    await dispatchThemeChanged(theme as "system" | "light" | "dark");
    messageApi.success("主题已切换");
  };

  const handleSaved = () => messageApi.success("已保存");

  return (
    <>
      {contextHolder}
      <SettingsPageShell>
        <SettingsSection title="主题" style={{ marginTop: 0 }}>
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
        </SettingsSection>

        <SettingsSection
          title="悬浮球"
          description="桌面上的圆形助手图标，点击展开悬浮窗"
        >
          <OrbBallSection onSaved={handleSaved} />
        </SettingsSection>

        <SettingsSection title="悬浮窗">
          <OrbPanelSection onSaved={handleSaved} />
        </SettingsSection>
      </SettingsPageShell>
    </>
  );
};
