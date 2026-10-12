import { Form } from "antd";
import { SettingsGroup } from "./SettingsGroup";
import { ORB_PANEL_SETTINGS } from "../core/fieldDefinitions";
import { useConfPatchHandlers } from "../core/useConfPatchHandlers";

export const OrbPanelSection: React.FC = () => {
  const { onPatch, onPatchComplete } = useConfPatchHandlers("orb");

  return (
    <Form layout="vertical">
      <SettingsGroup
        group="orb"
        definitions={ORB_PANEL_SETTINGS}
        onPatch={onPatch}
        onPatchComplete={onPatchComplete}
      />
    </Form>
  );
};
