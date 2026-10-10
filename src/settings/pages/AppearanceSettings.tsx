import { useEffect } from "react";
import { Form, Radio, message } from "antd";
import { useSettingsStore } from "@/stores/settingsStore";
import { MascotSection } from "./MascotSection";
import { OrbBallSection } from "./OrbBallSection";
import { OrbPanelSection } from "./OrbPanelSection";
import { SettingsSection } from "../shared/SettingsSection";
import { SettingsPageShell } from "../shared/SettingsPageShell";
import { dispatchConfPatch, dispatchThemeChanged } from "../core/settingsDispatcher";

export const AppearanceSettings: React.FC = () => {
  const appConf = useSettingsStore((s) => s.appConf);
  const loadAppConf = useSettingsStore((s) => s.loadAppConf);
  const loadOrbConf = useSettingsStore((s) => s.loadOrbConf);
  const loadMascotConf = useSettingsStore((s) => s.loadMascotConf);
  const [messageApi, contextHolder] = message.useMessage();

  useEffect(() => {
    loadAppConf();
    loadOrbConf();
    loadMascotConf();
  }, [loadAppConf, loadOrbConf, loadMascotConf]);

  const handleThemeChange = async (theme: string) => {
    await dispatchConfPatch("app", { theme });
    await dispatchThemeChanged(theme as "system" | "light" | "dark");
    messageApi.success("主题已切换");
  };

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
          title="助手形象"
          description="选择吉祥物的渲染方式、颜色和专属设置"
        >
          <MascotSection />
        </SettingsSection>

        <SettingsSection
          title="悬浮球"
          description="桌面上的圆形助手图标，点击展开悬浮窗"
        >
          <OrbBallSection />
        </SettingsSection>

        <SettingsSection title="悬浮窗">
          <OrbPanelSection />
        </SettingsSection>
      </SettingsPageShell>
    </>
  );
};