import React, { useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

export const OrbContextMenu: React.FC<{
  children: React.ReactNode;
}> = ({ children }) => {
  const handleContextMenu = useCallback(async (e: React.MouseEvent) => {
    e.preventDefault();
    try {
      await invoke("show_orb_context_menu", { x: e.clientX, y: e.clientY });
    } catch (err) {
      console.error(err);
    }
  }, []);

  return <div onContextMenu={handleContextMenu}>{children}</div>;
};