import { useEffect, useState } from "react";
import { Button, Tooltip } from "antd";
import { PushpinOutlined, PushpinFilled } from "@ant-design/icons";
import { useSettingsStore } from "@/stores/settingsStore";
import { dispatchConfPatch } from "@/settings/core/settingsDispatcher";
import { useTranslation } from "react-i18next";

export const PinToggle: React.FC = () => {
  const appConf = useSettingsStore((s) => s.appConf);
  const [pinned, setPinned] = useState(appConf?.stay_on_top ?? false);
  const { t } = useTranslation();

  useEffect(() => {
    if (appConf) setPinned(appConf.stay_on_top);
  }, [appConf]);

  const handleToggle = () => {
    const next = !pinned;
    setPinned(next);
    void dispatchConfPatch("app", { stay_on_top: next });
  };

  return (
    <Tooltip
      title={pinned ? t("common.actions.unpin") : t("common.actions.pin")}
      mouseEnterDelay={0.5}
    >
      <Button
        type="text"
        size="small"
        icon={pinned ? <PushpinFilled /> : <PushpinOutlined />}
        onClick={handleToggle}
        style={{ width: 32, height: 28 }}
      />
    </Tooltip>
  );
};
