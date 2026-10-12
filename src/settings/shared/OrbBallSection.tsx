import { useEffect } from "react";
import { Form } from "antd";
import { useSettingsStore } from "@/stores/settingsStore";
import { SettingsGroup } from "./SettingsGroup";
import {
  ORB_BALL_SETTINGS,
  MASCOT_RENDERER_SETTING,
  MASCOT_COLOR_SETTING,
} from "../core/fieldDefinitions";
import { getDefinitionsByGroup } from "../core/fieldRegistry";
import { useConfPatchHandlers } from "../core/useConfPatchHandlers";

const MASCOT_FIXED_SETTINGS = [MASCOT_RENDERER_SETTING, MASCOT_COLOR_SETTING];

function getMascotRendererSettings() {
  const all = getDefinitionsByGroup("mascot");
  const fixedKeys = new Set(MASCOT_FIXED_SETTINGS.map((d) => d.key));
  return all.filter((d) => !fixedKeys.has(d.key));
}

const MASCOT_ALL_SETTINGS = [
  ...MASCOT_FIXED_SETTINGS,
  ...getMascotRendererSettings(),
];

export const OrbBallSection: React.FC = () => {
  const loadOrbConf = useSettingsStore((s) => s.loadOrbConf);
  const loadMascotConf = useSettingsStore((s) => s.loadMascotConf);
  const { onPatch, onPatchComplete } = useConfPatchHandlers("orb");

  useEffect(() => {
    loadOrbConf();
    loadMascotConf();
  }, [loadOrbConf, loadMascotConf]);

  return (
    <>
      <Form layout="vertical">
        <SettingsGroup group="mascot" definitions={MASCOT_ALL_SETTINGS} />
      </Form>
      <Form layout="vertical">
        <SettingsGroup
          group="orb"
          definitions={ORB_BALL_SETTINGS}
          onPatch={onPatch}
          onPatchComplete={onPatchComplete}
        />
      </Form>
    </>
  );
};
