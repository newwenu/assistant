import { useEffect } from "react";
import { Form } from "antd";
import { useSettingsStore } from "@/stores/settingsStore";
import { SettingsGroup } from "../shared/layout/SettingsGroup";
import { SettingsSection } from "../shared/layout/SettingsSection";
import { SettingsPageShell } from "../shared/layout/SettingsPageShell";
import { APP_SETTINGS, COMPACT_SETTINGS } from "../core/field/fieldDefinitions";
import { useConfPatchHandlers } from "../core/useConfPatchHandlers";

export const GeneralSettings: React.FC = () => {
  const loadAppConf = useSettingsStore((s) => s.loadAppConf);
  const { onPatch, onPatchComplete } = useConfPatchHandlers("app");

  useEffect(() => {
    void loadAppConf();
  }, [loadAppConf]);

  return (
    <SettingsPageShell>
      <SettingsSection id="general-main" style={{ marginTop: 0 }}>
        <Form layout="vertical">
          <SettingsGroup
            group="app"
            definitions={APP_SETTINGS}
            onPatch={onPatch}
            onPatchComplete={onPatchComplete}
          />
          <SettingsGroup
            group="app"
            definitions={COMPACT_SETTINGS}
            onPatch={onPatch}
            onPatchComplete={onPatchComplete}
          />
        </Form>
      </SettingsSection>
    </SettingsPageShell>
  );
};
