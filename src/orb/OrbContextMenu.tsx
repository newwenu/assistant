import React from "react";
import { Dropdown, MenuProps } from "antd";
import { invoke } from "@tauri-apps/api/core";

const items: MenuProps["items"] = [
  {
    key: "new_chat",
    label: "新建对话",
    disabled: true,
  },
  {
    key: "open_main",
    label: "打开主窗口",
  },
  { type: "divider" },
  {
    key: "quit",
    label: "退出",
    danger: true,
  },
];

const handleClick: MenuProps["onClick"] = ({ key }) => {
  switch (key) {
    case "open_main":
      invoke("open_main_window").catch(console.error);
      break;
    case "quit":
      invoke("app_quit").catch(console.error);
      break;
  }
};

export const OrbContextMenu: React.FC<{
  children: React.ReactNode;
}> = ({ children }) => {
  return (
    <Dropdown menu={{ items, onClick: handleClick }} trigger={["contextMenu"]}>
      {children}
    </Dropdown>
  );
};
