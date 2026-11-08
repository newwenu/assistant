import React from "react";
import { Button, Tooltip } from "antd";
import { ToolOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import type { ConversationToolConfig } from "../core/chatTypes";
import css from "./ToolToolbar.module.css";

interface ToolToolbarProps {
  config: ConversationToolConfig;
  /** 清单总条数，用于展示「已启用 / 全部」。 */
  total: number;
  open: boolean;
  onToggle: () => void;
}

/**
 * 会话工具概览条（I38d §4.7）：位于输入区工具栏，点击展开装配面板。
 *
 * 只读展示「已启用数量 + 层级上限」，不承载编辑逻辑；编辑全部在面板内完成。
 */
export const ToolToolbar: React.FC<ToolToolbarProps> = ({
  config,
  total,
  open,
  onToggle,
}) => {
  const { t } = useTranslation();
  const enabled = config.enabledTools.length;
  const summary = t("chat.tool.toolbar.summary", {
    enabled,
    total,
    tier: t(`chat.tool.approval.tier.${config.maxToolTier}`),
  });

  return (
    <Tooltip title={t("chat.tool.toolbar.hint")}>
      <Button
        size="small"
        type={open ? "primary" : "text"}
        icon={<ToolOutlined />}
        onClick={onToggle}
        className={css.chip}
      >
        <span className={css.label}>{summary}</span>
      </Button>
    </Tooltip>
  );
};
