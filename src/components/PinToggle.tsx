import { useEffect, useState } from "react";
import { Button, Tooltip } from "antd";
import { PushpinOutlined, PushpinFilled } from "@ant-design/icons";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useSettingsStore } from "@/stores/settingsStore";
import type { AppConf } from "@/stores/settingsStore";

export const PinToggle: React.FC = () => {
  const appConf = useSettingsStore((s) => s.appConf);
  const saveAppConf = useSettingsStore((s) => s.saveAppConf);
  const [pinned, setPinned] = useState(appConf?.stay_on_top ?? false);

  useEffect(() => {
    if (appConf) setPinned(appConf.stay_on_top);
  }, [appConf]);

  const handleToggle = async () => {
    const next = !pinned;
    setPinned(next);
    try {
      await getCurrentWindow().setAlwaysOnTop(next);
    } catch (e) {
      console.error("[PinToggle] setAlwaysOnTop failed:", e);
    }
    if (appConf) {
      const newConf: AppConf = { ...appConf, stay_on_top: next };
      await saveAppConf(newConf);
    }
  };

  return (
    <Tooltip title={pinned ? "取消置顶" : "置顶"} mouseEnterDelay={0.5}>
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
