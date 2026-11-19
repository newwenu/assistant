import React from "react";
import { Button, Tooltip } from "antd";
import { ToolOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import type { ConversationToolConfig } from "../core/chatTypes";
import css from "./ToolToolbar.module.css";

interface ToolToolbarProps {
  config: ConversationToolConfig;
  /** 已启用的可勾选条目数（伴随项不计）。 */
  enabled: number;
  /** 可勾选条目总数（伴随项不计）。 */
  total: number;
  open: boolean;
  onToggle: () => void;
}

/**
 * 会话工具概览条（I38d §4.7 / I47 §5.1）：位于输入区工具栏，点击展开装配面板。
 *
 * 只读展示「审批档位 + 已启用数量 + 层级上限」，不承载编辑逻辑；编辑全部在面板内完成。
 * 档位段固定不省略，保证收起面板时也能一眼看到当前审批级别。
 */
export const ToolToolbar: React.FC<ToolToolbarProps> = ({
  config,
  enabled,
  total,
  open,
  onToggle,
}) => {
  const { t } = useTranslation();
  const levelLabel = t(`chat.tool.approval.level.${config.approvalLevel}`);
  const summary = t("chat.tool.toolbar.summary", {
    enabled,
    total,
    tier: t(`chat.tool.approval.tier.${config.maxToolTier}`),
  });
  const hint = t("chat.tool.toolbar.hint", {
    level: t(`chat.tool.approval.level.${config.approvalLevel}`),
    hint: t(`chat.tool.approval.level_hint.${config.approvalLevel}`),
  });

  return (
    <Tooltip title={hint}>
      <Button
        size="small"
        type={open ? "primary" : "text"}
        icon={<ToolOutlined />}
        onClick={onToggle}
        className={css.chip}
        data-tool-panel-anchor
      >
        <span className={css.level}>{levelLabel}</span>
        <span className={css.sep}>·</span>
        <span className={css.label}>{summary}</span>
      </Button>
    </Tooltip>
  );
};
