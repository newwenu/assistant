import React from "react";
import { useTranslation } from "react-i18next";
import { Button, Tooltip } from "antd";
import { MenuFoldOutlined, MenuUnfoldOutlined } from "@ant-design/icons";
import css from "./HistoryDetail.module.css";

type ToggleRef = HTMLButtonElement | HTMLAnchorElement;

interface HistorySidebarToggleProps {
  collapsed: boolean;
  disabled?: boolean;
  onToggle: () => void;
}

export const HistorySidebarToggle = React.forwardRef<
  ToggleRef,
  HistorySidebarToggleProps
>(({ collapsed, disabled = false, onToggle }, ref) => {
  const { t } = useTranslation();
  const label = collapsed
    ? t("history.expand_sidebar")
    : t("history.collapse_sidebar");

  return (
    <Tooltip title={label}>
      <span className={css.toggleWrap}>
        <Button
          ref={ref}
          type="text"
          size="small"
          disabled={disabled}
          aria-label={label}
          aria-expanded={!collapsed}
          aria-controls="history-sidebar"
          onClick={onToggle}
          icon={collapsed ? <MenuUnfoldOutlined /> : <MenuFoldOutlined />}
        />
      </span>
    </Tooltip>
  );
});

HistorySidebarToggle.displayName = "HistorySidebarToggle";
