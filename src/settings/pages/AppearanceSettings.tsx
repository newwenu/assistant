import { useEffect } from "react";
import { Form } from "antd";
import { useSettingsStore } from "@/stores/settingsStore";
import { OrbBallSection } from "../shared/OrbBallSection";
import { OrbPanelSection } from "../shared/OrbPanelSection";
import { SettingsSection } from "../shared/SettingsSection";
import { SettingsPageShell } from "../shared/SettingsPageShell";
import { SettingsGroup } from "../shared/SettingsGroup";
import { THEME_SETTING } from "../core/fieldDefinitions";

export const AppearanceSettings: React.FC = () => {
  const loadAppConf = useSettingsStore((s) => s.loadAppConf);
  const loadOrbConf = useSettingsStore((s) => s.loadOrbConf);
  const loadMascotConf = useSettingsStore((s) => s.loadMascotConf);

  useEffect(() => {
    void loadAppConf();
    void loadOrbConf();
    void loadMascotConf();
  }, [loadAppConf, loadOrbConf, loadMascotConf]);

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
