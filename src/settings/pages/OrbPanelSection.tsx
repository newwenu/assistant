import { Form } from "antd";
import { SettingsGroup } from "../shared/SettingsGroup";
import { ORB_PANEL_SETTINGS } from "../core/definitions";
import { useConfPatchHandlers } from "../core/useConfPatchHandlers";

export const OrbPanelSection: React.FC = () => {
  const { onPatch } = useConfPatchHandlers("orb");

  return (
    <Form layout="vertical">
      <SettingsGroup
        group="orb"
        definitions={ORB_PANEL_SETTINGS}
        onPatch={onPatch}
      />
    </Form>
  );
};