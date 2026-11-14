import React from "react";
import { DownOutlined, RightOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import css from "./GroupHeader.module.css";

interface GroupHeaderProps {
  steps: number;
  /** run 内的思考行数：与工具步共用同一展开开关，但单独计数。 */
  reasoning: number;
  failed: number;
  durationMs: number | null;
  expanded: boolean;
  onToggle: () => void;
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

/**
 * 过程组组头（I46 §5.4）：只展示计数与展开控件，不读正文。
 * 展开态由调用方（`TurnGroup`）持有，本组件纯展示。
 */
export const GroupHeader: React.FC<GroupHeaderProps> = ({
  steps,
  reasoning,
  failed,
  durationMs,
  expanded,
  onToggle,
}) => {
  const { t } = useTranslation();
  return (
    <button className={css.root} onClick={onToggle} type="button">
      <span className={css.chevron}>
        {expanded ? <DownOutlined /> : <RightOutlined />}
      </span>
      <span className={css.label}>{t("chat.message.group.process")}</span>
      {steps > 0 && (
        <span className={css.count}>
          {t("chat.message.group.steps", { count: steps })}
        </span>
      )}
      {reasoning > 0 && (
        <span className={css.count}>
          {t("chat.message.group.thoughts", { count: reasoning })}
        </span>
      )}
      {failed > 0 && (
        <span className={css.failed}>
          {t("chat.message.group.failed", { count: failed })}
        </span>
      )}
      {durationMs !== null && (
        <span className={css.duration}>{formatDuration(durationMs)}</span>
      )}
    </button>
  );
};
