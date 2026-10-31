import { useEffect } from "react";
import { Form } from "antd";
import { useSettingsStore } from "@/stores/settingsStore";
import { SettingsPageShell } from "@/settings/shared/layout/SettingsPageShell";
import { SettingsSection } from "@/settings/shared/layout/SettingsSection";
import { SettingsGroup } from "@/settings/shared/sections/SettingsGroup";
import { getDefinitionsBySection } from "@/settings/core/field/fieldRegistry";
import { useConfPatchHandlers } from "@/settings/core/useConfPatchHandlers";

export const ProbeSettingsPage: React.FC = () => {
  const loadConf = useSettingsStore((s) => s.loadConf);
  const { onPatch, onPatchComplete } = useConfPatchHandlers("probe");

  useEffect(() => {
    void loadConf("probe");
  }, [loadConf]);

  return (
    <SettingsPageShell>
      <SettingsSection id="probe-general" style={{ marginTop: 0 }}>
        <Form layout="vertical">
          <SettingsGroup
            group="probe"
            definitions={getDefinitionsBySection("probe-general")}
            onPatch={onPatch}
            onPatchComplete={onPatchComplete}
          />
        </Form>
      </SettingsSection>
    </SettingsPageShell>
  );
};
