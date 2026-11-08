import { useEffect } from "react";
import { Form } from "antd";
import { useSettingsStore } from "@/stores/settingsStore";
import { OrbBallSection } from "../shared/sections/OrbBallSection";
import { OrbPanelSection } from "../shared/sections/OrbPanelSection";
import { SettingsSection } from "../shared/layout/SettingsSection";
import { SettingsPageShell } from "../shared/layout/SettingsPageShell";
import { SettingsGroup } from "../shared/sections/SettingsGroup";
import { THEME_SETTING } from "../core/field/fieldDefinitions";

export const AppearanceSettings: React.FC = () => {
  const loadConf = useSettingsStore((s) => s.loadConf);

  useEffect(() => {
    void loadConf("app");
    void loadConf("orb");
    void loadConf("mascot");
  }, [loadConf]);

  return (
    <SettingsPageShell>
      <SettingsSection id="theme" style={{ marginTop: 0 }}>
        <Form layout="vertical">
          <SettingsGroup group="app" definitions={[THEME_SETTING]} />
        </Form>
      </SettingsSection>

      <SettingsSection id="orb-ball">
        <OrbBallSection />
      </SettingsSection>

      <SettingsSection id="orb-panel">
        <OrbPanelSection />
      </SettingsSection>
    </SettingsPageShell>
  );
};
