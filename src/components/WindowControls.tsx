import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Button, Tooltip } from "antd";
import {
  MinusOutlined,
  BorderOutlined,
  BlockOutlined,
  CloseOutlined,
} from "@ant-design/icons";

export const WindowControls: React.FC = () => {
  const [isMax, setIsMax] = useState(false);

  useEffect(() => {
    const win = getCurrentWindow();
    win.isMaximized().then((max) => setIsMax(!!max));
  }, []);

  const handleMinimize = () => {
    getCurrentWindow().minimize();
  };

  const handleToggleMaximize = async () => {
    const win = getCurrentWindow();
    await win.toggleMaximize();
    const max = await win.isMaximized();
    setIsMax(!!max);
  };

  const handleClose = () => {
    getCurrentWindow().close();
  };

  return (
    <div style={{ display: "flex", alignItems: "center", gap: 2 }}>
      <Tooltip title="最小化" mouseEnterDelay={0.5}>
        <Button
          type="text"
          size="small"
          icon={<MinusOutlined />}
          onClick={handleMinimize}
          style={{ width: 32, height: 28 }}
        />
      </Tooltip>
      <Tooltip title={isMax ? "还原" : "最大化"} mouseEnterDelay={0.5}>
        <Button
          type="text"
          size="small"
          icon={isMax ? <BlockOutlined /> : <BorderOutlined />}
          onClick={handleToggleMaximize}
          style={{ width: 32, height: 28 }}
        />
      </Tooltip>
      <Tooltip title="关闭" mouseEnterDelay={0.5}>
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
