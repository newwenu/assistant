import { Form } from "antd";
import { SettingsGroup } from "./SettingsGroup";
import { ORB_PANEL_SETTINGS } from "./definitions";
import { dispatchConfPatch } from "./settingsDispatcher";

interface OrbPanelSectionProps {
  onSaved?: () => void;
}

export const OrbPanelSection: React.FC<OrbPanelSectionProps> = ({
  onSaved,
}) => {
  const handlePatch = async (patch: Record<string, unknown>) => {
    await dispatchConfPatch("orb", patch);
    onSaved?.();
  };

  return (
    <Form layout="vertical">
      <SettingsGroup
        group="orb"
        definitions={ORB_PANEL_SETTINGS}
        onPatch={handlePatch}
      />
    </Form>
  );
};
