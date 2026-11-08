import React from "react";
import { Tooltip } from "antd";
import { useTranslation } from "react-i18next";
import type { ContextUsage } from "../core/chatTypes";
import {
  CONTEXT_WARN_THRESHOLD,
  CONTEXT_DANGER_THRESHOLD,
} from "../../config/constants";

function formatTokenCount(n: number) {
  return n >= 1000 ? `${(n / 1000).toFixed(1)}k` : String(Math.round(n));
}

export const ContextBar: React.FC<{ usage: ContextUsage }> = ({ usage }) => {
  const { t } = useTranslation();

  const withEstimate = (n: number) =>
    `${usage.estimated ? "~" : ""}${formatTokenCount(n)}`;
  const num = (n: number) =>
    `${usage.estimated ? "~" : ""}${Math.round(n).toLocaleString()}`;

  const sourceLabel = (): string => {
    if (!usage.estimated) return t("chat.context.source_provider");
    if (usage.estimatedSource === "projection")
      return t("chat.context.source_estimate_projection");
    return t("chat.context.source_estimate_record");
  };

  const tooltipLines: string[] = [];
  if (usage.hasUsage) {
    tooltipLines.push(`${t("chat.context.used")}: ${num(usage.usedTokens)}`);
    tooltipLines.push(`${t("chat.context.source")}: ${sourceLabel()}`);
    if (usage.completionTokens !== undefined) {
      tooltipLines.push(
        `${t("chat.context.output")}: ${num(usage.completionTokens)}`,
      );
    }
    if (usage.cacheReadTokens !== undefined) {
      tooltipLines.push(
        `${t("chat.context.cache_read")}: ${num(usage.cacheReadTokens)}`,
      );
    }
    if (usage.cacheWriteTokens !== undefined) {
      tooltipLines.push(
        `${t("chat.context.cache_write")}: ${num(usage.cacheWriteTokens)}`,
      );
    }
    if (usage.reasoningTokens !== undefined) {
      tooltipLines.push(
        `${t("chat.context.reasoning")}: ${num(usage.reasoningTokens)}`,
      );
    }
  } else {
    tooltipLines.push(`${t("chat.context.used")}: 0`);
    tooltipLines.push(
      `${t("chat.context.source")}: ${t("chat.context.no_messages")}`,
    );
  }
  if (usage.cumulativeTokens !== undefined) {
    tooltipLines.push(
      `${t("chat.context.cumulative")}: ${num(usage.cumulativeTokens)}`,
    );
  }

  // 分支无消息：窗口已知时占用确定为 0，仍展示 `0 / 窗口`；窗口未知才退化为「—」
  if (!usage.hasUsage && usage.maxTokens === undefined) {
    return (
      <div className="chat-context-bar">
        <span className="chat-context-bar-text">—</span>
      </div>
    );
  }

  const tooltipTitle = tooltipLines.join("\n");

  if (usage.maxTokens === undefined) {
    return (
      <Tooltip
        title={<span style={{ whiteSpace: "pre-line" }}>{tooltipTitle}</span>}
      >
        <div className="chat-context-bar">
          <span className="chat-context-bar-text">
            {withEstimate(usage.usedTokens)}
          </span>
        </div>
      </Tooltip>
    );
  }

  const window = usage.maxTokens;
  const pct = Math.min((usage.percentage ?? 0) * 100, 100);
  let status: "normal" | "exception" | "active" = "normal";
  if (pct >= CONTEXT_DANGER_THRESHOLD * 100) status = "exception";
  else if (pct >= CONTEXT_WARN_THRESHOLD * 100) status = "active";

  const used = usage.usedTokens;
  let hit = usage.cacheReadTokens ?? 0;
  let write = usage.cacheWriteTokens ?? 0;
  if (hit + write > used && hit + write > 0) {
    const scale = used / (hit + write);
    hit *= scale;
    write *= scale;
  }
  const uncached = Math.max(used - hit - write, 0);

  const statusColor =
    status === "exception" ? "#ff4d4f" : status === "active" ? "#faad14" : null;
  const segColor = (normal: string) => statusColor ?? normal;

  const segments = [
    { key: "hit", width: hit, color: segColor("#52c41a") },
    { key: "write", width: write, color: segColor("#13c2c2") },
    { key: "uncached", width: uncached, color: segColor("#1677ff") },
  ];

  return (
    <Tooltip
      title={<span style={{ whiteSpace: "pre-line" }}>{tooltipTitle}</span>}
    >
      <div className="chat-context-bar">
        <span className="chat-context-bar-text">
          {withEstimate(used)}/{formatTokenCount(window)}
        </span>
        <div className="chat-context-segments">
          {segments.map((s) => (
            <div
              key={s.key}
              className="chat-context-segment"
              style={{
                width: `${(s.width / window) * 100}%`,
                background: s.color,
                display: s.width > 0 ? "block" : "none",
              }}
            />
          ))}
        </div>
        <span className="chat-context-bar-text">{pct.toFixed(0)}%</span>
      </div>
    </Tooltip>
  );
};
