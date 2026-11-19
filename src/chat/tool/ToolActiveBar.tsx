import React from "react";
import { LoadingOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import type { ToolCallView } from "../core/chatTypes";
import { formatElapsed } from "./toolCallDisplay";
import { useNowTick } from "./useNowTick";
import css from "./ToolActiveBar.module.css";

interface ToolActiveBarProps {
  view: ToolCallView;
}

/**
 * 全局活动指示器（I60 §3.7）：回答「现在还在跑什么」，不随滚动消失。
 * 与工具卡片允许重复：卡片回答「这一步做了什么」。
 */
export const ToolActiveBar: React.FC<ToolActiveBarProps> = ({ view }) => {
  const { t } = useTranslation();
  const now = useNowTick(true);
  const time =
    view.startedAt == null ? null : formatElapsed(now - view.startedAt);
  const name = view.name.trim();
  // 指示器只承载 `preparing` / `running`（其余状态不会进入 `pickActiveTool`）。
  const status = view.status === "preparing" ? "preparing" : "running";
  const namedKey = `chat.tool.active_bar.${status}` as const;
  const anonymousKey = `chat.tool.active_bar.${status}_anonymous` as const;
  const text =
    time == null
      ? name || t(`chat.tool.status.${status}`)
      : t(name ? namedKey : anonymousKey, { name, time });

  return (
    <div className={css.root} role="status" aria-live="polite">
      <span className={css.icon}>
        <LoadingOutlined spin />
      </span>
      <span className={css.text}>{text}</span>
    </div>
  );
};
