import { useEffect } from "react";
import { Form } from "antd";
import { useSettingsStore } from "@/stores/settingsStore";
import { SettingsGroup } from "../shared/sections/SettingsGroup";
import { SettingsSection } from "../shared/layout/SettingsSection";
import { SettingsPageShell } from "../shared/layout/SettingsPageShell";
import {
  VAULT_GENERAL_SETTINGS,
  VAULT_SSH_SETTINGS,
} from "../core/field/fieldDefinitions";
import { useConfPatchHandlers } from "../core/useConfPatchHandlers";

export const VaultSettingsPage: React.FC = () => {
  const loadAppConf = useSettingsStore((s) => s.loadAppConf);
  const { onPatch, onPatchComplete } = useConfPatchHandlers("app");

  useEffect(() => {
    void loadAppConf();
  }, [loadAppConf]);

  return (
    <SettingsPageShell>
      <SettingsSection id="vault-general" style={{ marginTop: 0 }}>
        <Form layout="vertical">
          <SettingsGroup
            group="app"
            definitions={VAULT_GENERAL_SETTINGS}
            onPatch={onPatch}
            onPatchComplete={onPatchComplete}
          />
        </Form>
      </SettingsSection>

      <SettingsSection id="vault-ssh">
        <Form layout="vertical">
          <SettingsGroup
            group="app"
            definitions={VAULT_SSH_SETTINGS}
            onPatch={onPatch}
            onPatchComplete={onPatchComplete}
          />
        </Form>
      </SettingsSection>
    </SettingsPageShell>
  );
};
