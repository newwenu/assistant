import { useEffect, useState } from "react";
import { Button, Tooltip } from "antd";
import { PushpinOutlined, PushpinFilled } from "@ant-design/icons";
import { useConfGroup } from "@/settings/core/runtime/useConfGroup";
import { dispatchConfPatch } from "@/settings/core/runtime/settingsDispatcher";
import { useSettingMessage } from "@/settings/shared/useSettingMessage";
import { useTranslation } from "react-i18next";

export const PinToggle: React.FC = () => {
  const appConf = useConfGroup("app");
  const [pinned, setPinned] = useState(appConf?.stayOnTop ?? false);
  const { t } = useTranslation();
  const msg = useSettingMessage();

  useEffect(() => {
    if (appConf) setPinned(appConf.stayOnTop);
  }, [appConf]);

  const handleToggle = () => {
    const prev = pinned;
    const next = !prev;
    setPinned(next);
    void dispatchConfPatch("app", { stayOnTop: next }).catch((e: unknown) => {
      setPinned(prev);
      msg.showFailed(e);
    });
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
