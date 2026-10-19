import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Button, Tooltip } from "antd";
import {
  MinusOutlined,
  BorderOutlined,
  BlockOutlined,
  CloseOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";

export const WindowControls: React.FC = () => {
  const [isMax, setIsMax] = useState(false);
  const { t } = useTranslation();

  useEffect(() => {
    const win = getCurrentWindow();
    void win.isMaximized().then((max) => setIsMax(!!max));
  }, []);

  const handleMinimize = () => {
    void getCurrentWindow().minimize();
  };

  const handleToggleMaximize = () => {
    void (async () => {
      const win = getCurrentWindow();
      await win.toggleMaximize();
      const max = await win.isMaximized();
      setIsMax(!!max);
    })();
  };

  const handleClose = () => {
    void getCurrentWindow().close();
  };

  return (
    <div style={{ display: "flex", alignItems: "center", gap: 2 }}>
      <Tooltip title={t("common.actions.minimize")} mouseEnterDelay={0.5}>
        <Button
          type="text"
          size="small"
          icon={<MinusOutlined />}
          onClick={handleMinimize}
          style={{ width: 32, height: 28 }}
        />
      </Tooltip>
      <Tooltip
        title={
          isMax ? t("common.actions.restore") : t("common.actions.maximize")
        }
        mouseEnterDelay={0.5}
      >
        <Button
          type="text"
          size="small"
          icon={isMax ? <BlockOutlined /> : <BorderOutlined />}
          onClick={handleToggleMaximize}
          style={{ width: 32, height: 28 }}
        />
      </Tooltip>
      <Tooltip title={t("common.actions.close")} mouseEnterDelay={0.5}>
        <Button
          type="text"
          size="small"
          icon={<CloseOutlined />}
          onClick={handleClose}
          style={{ width: 32, height: 28 }}
        />
      </Tooltip>
    </div>
  );
};
