import { useEffect } from "react";
import { Form } from "antd";
import { useSettingsStore } from "@/stores/settingsStore";
import { SettingsGroup } from "../shared/SettingsGroup";
import { SettingsSection } from "../shared/SettingsSection";
import { SettingsPageShell } from "../shared/SettingsPageShell";
import { APP_SETTINGS } from "../core/definitions";
import { useConfPatchHandlers } from "../core/useConfPatchHandlers";

export const GeneralSettings: React.FC = () => {
  const loadAppConf = useSettingsStore((s) => s.loadAppConf);
  const { onPatch, onPatchComplete } = useConfPatchHandlers("app");

  useEffect(() => {
    loadAppConf();
  }, [loadAppConf]);

  return (
    <SettingsPageShell>
      <SettingsSection title="通用设置" style={{ marginTop: 0 }}>
        <Form layout="vertical">
          <SettingsGroup
            group="app"
            definitions={APP_SETTINGS}
            onPatch={onPatch}
            onPatchComplete={onPatchComplete}
          />
        </Form>
      </SettingsSection>
    </SettingsPageShell>
  );
};