import { useEffect } from "react";
import { Form } from "antd";
import { useSettingsStore } from "@/stores/settingsStore";
import { SettingsGroup } from "../shared/SettingsGroup";
import { ORB_BALL_SETTINGS } from "../core/definitions";
import { useConfPatchHandlers } from "../core/useConfPatchHandlers";

export const OrbBallSection: React.FC = () => {
  const loadOrbConf = useSettingsStore((s) => s.loadOrbConf);
  const { onPatch, onPatchComplete } = useConfPatchHandlers("orb");

  useEffect(() => {
    loadOrbConf();
  }, [loadOrbConf]);

  return (
    <Form layout="vertical">
      <SettingsGroup
        group="orb"
        definitions={ORB_BALL_SETTINGS}
        onPatch={onPatch}
        onPatchComplete={onPatchComplete}
      />
    </Form>
  );
};