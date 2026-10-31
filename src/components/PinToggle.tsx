import { useEffect, useState } from "react";
import { Button, Tooltip } from "antd";
import { PushpinOutlined, PushpinFilled } from "@ant-design/icons";
import { useConfGroup } from "@/settings/core/useConfGroup";
import { dispatchConfPatch } from "@/settings/core/settingsDispatcher";
import { useTranslation } from "react-i18next";

export const PinToggle: React.FC = () => {
  const appConf = useConfGroup("app");
  const [pinned, setPinned] = useState(appConf?.stayOnTop ?? false);
  const { t } = useTranslation();

  useEffect(() => {
    if (appConf) setPinned(appConf.stayOnTop);
  }, [appConf]);

  const handleToggle = () => {
    const next = !pinned;
    setPinned(next);
    void dispatchConfPatch("app", { stayOnTop: next });
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
