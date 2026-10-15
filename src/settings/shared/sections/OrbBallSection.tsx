import { useEffect, useMemo } from "react";
import { Form } from "antd";
import { useSettingsStore } from "@/stores/settingsStore";
import { SettingsGroup } from "../layout/SettingsGroup";
import {
  ORB_BALL_SETTINGS,
  MASCOT_RENDERER_SETTING,
  MASCOT_COLOR_SETTING,
} from "../../core/field/fieldDefinitions";
import { getDefinitionsByGroup } from "../../core/field/fieldRegistry";
import { useConfPatchHandlers } from "../../core/useConfPatchHandlers";

const MASCOT_FIXED_SETTINGS = [MASCOT_RENDERER_SETTING, MASCOT_COLOR_SETTING];

export const OrbBallSection: React.FC = () => {
  const loadOrbConf = useSettingsStore((s) => s.loadOrbConf);
  const loadMascotConf = useSettingsStore((s) => s.loadMascotConf);
  const { onPatch, onPatchComplete } = useConfPatchHandlers("orb");

  const mascotAllSettings = useMemo(() => {
    const all = getDefinitionsByGroup("mascot");
    const fixedKeys = new Set(MASCOT_FIXED_SETTINGS.map((d) => d.key));
    const dynamic = all.filter((d) => !fixedKeys.has(d.key));
    return [...MASCOT_FIXED_SETTINGS, ...dynamic];
  }, []);

  useEffect(() => {
    void loadOrbConf();
    void loadMascotConf();
  }, [loadOrbConf, loadMascotConf]);

  return (
    <Form layout="vertical">
      <SettingsGroup group="mascot" definitions={mascotAllSettings} />
      <SettingsGroup
        group="orb"
        definitions={ORB_BALL_SETTINGS}
        onPatch={onPatch}
        onPatchComplete={onPatchComplete}
      />
    </Form>
  );
};
