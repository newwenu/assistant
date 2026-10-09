import { useEffect } from "react";
import { Form } from "antd";
import { useSettingsStore } from "@/stores/settingsStore";
import { SettingsGroup } from "./SettingsGroup";
import { SettingsSection } from "./SettingsSection";
import { SettingsPageShell } from "./SettingsPageShell";
import { APP_SETTINGS } from "./definitions";

export const GeneralSettings: React.FC = () => {
  const loadAppConf = useSettingsStore((s) => s.loadAppConf);

  useEffect(() => {
    loadAppConf();
  }, [loadAppConf]);

  return (
    <SettingsPageShell>
      <SettingsSection title="通用设置" style={{ marginTop: 0 }}>
        <Form layout="vertical">
          <SettingsGroup group="app" definitions={APP_SETTINGS} />
        </Form>
      </SettingsSection>
    </SettingsPageShell>
  );
};
