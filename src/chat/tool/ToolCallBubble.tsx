import React, { useState } from "react";
import {
  CheckCircleOutlined,
  CloseCircleOutlined,
  DownOutlined,
  ExclamationCircleOutlined,
  LoadingOutlined,
  QuestionCircleOutlined,
  StopOutlined,
  UpOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import type { ToolCallView, ToolCallStatus } from "../core/chatTypes";
import { formatBytes } from "@/utils/formatBytes";
import { ApprovalCard } from "./ApprovalCard";
import { QuestionCard } from "./QuestionCard";
import {
  formatToolTiming,
  resolveResultDisplay,
  resultStats,
  summarizeArguments,
} from "./toolCallDisplay";
import { useNowTick } from "./useNowTick";
import css from "./ToolCallBubble.module.css";

interface ToolCallBubbleProps {
  view: ToolCallView;
}

function StatusIcon({ status }: { status: ToolCallStatus }) {
  switch (status) {
    case "preparing":
    case "running":
      return <LoadingOutlined spin />;
    case "success":
      return <CheckCircleOutlined />;
    case "error":
      return <CloseCircleOutlined />;
    case "awaiting_user":
      return <ExclamationCircleOutlined />;
    case "cancelled":
      return <StopOutlined />;
    case "missing":
      return <QuestionCircleOutlined />;
  }
}

function statusClass(status: ToolCallStatus): string {
  switch (status) {
    case "preparing":
      return css.statusPreparing;
    case "running":
      return css.statusRunning;
    case "success":
      return css.statusSuccess;
    case "error":
      return css.statusError;
    case "awaiting_user":
      return css.statusAwaiting;
    case "cancelled":
      return css.statusCancelled;
    case "missing":
      return css.statusMissing;
  }
}

export const ToolCallBubble: React.FC<ToolCallBubbleProps> = ({ view }) => {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);

  const display = resolveResultDisplay(view.resultText, view.compact);
  const hasBody = display.body.trim().length > 0;
  const showBody = hasBody && (expanded || !display.defaultCollapsed);
  const collapsible = hasBody && display.defaultCollapsed;
  const argsSummary =
    (view.argsPreview?.trim() ? view.argsPreview.trim() : null) ??
    summarizeArguments(view.arguments) ??
    (view.argsText?.trim() ? view.argsText.trim() : null);
  const active = view.status === "preparing" || view.status === "running";
  const awaitingUser = view.status === "awaiting_user";
  const awaitingApproval = awaitingUser && view.approval !== null;
  const now = useNowTick(active);
  const duration = formatToolTiming(view, now);
  const stats =
    collapsible && display.source === "text" ? resultStats(display.body) : null;
  const truncated = display.truncated
    ? [
        scaleNumbers(display.truncated),
        t(`chat.tool.unit.${display.truncated.unit}`),
      ]
        .filter(Boolean)
        .join(" ")
    : null;

  return (
    <div className={`${css.root} ${statusClass(view.status)}`}>
      <div className={css.header}>
        <span className={css.statusIcon}>
          <StatusIcon status={view.status} />
        </span>
        {!awaitingApproval && <span className={css.name}>{view.name}</span>}
        <span className={css.statusLabel}>
          {t(`chat.tool.status.${view.status}`)}
        </span>
        {duration && <span className={css.duration}>{duration}</span>}
      </div>

      {!awaitingApproval && argsSummary && (
        <div className={css.args}>{argsSummary}</div>
      )}

      {awaitingUser && view.approval && (
        <ApprovalCard approval={view.approval} startedAt={view.startedAt} />
      )}

      {awaitingUser && view.question && (
        <QuestionCard question={view.question} startedAt={view.startedAt} />
      )}

      {display.head && <div className={css.head}>{display.head}</div>}

      {truncated && (
        <div className={css.scale}>
          {t("chat.tool.result_truncated", { scale: truncated })}
        </div>
      )}

      {display.persisted && (
        <div className={css.persisted}>
          {t("chat.tool.result_persisted")}
          <span className={css.persistedPath}>{display.persisted.path}</span>
          {display.persisted.bytes != null && (
            <span className={css.resultStats}>
              {formatBytes(display.persisted.bytes)}
            </span>
          )}
        </div>
      )}

      {showBody && <pre className={css.result}>{display.body}</pre>}

      {collapsible && (
        <button
          className={css.toggle}
          onClick={() => setExpanded((v) => !v)}
          type="button"
        >
          {expanded ? <UpOutlined /> : <DownOutlined />}
          <span>
            {expanded ? t("chat.tool.collapse") : t("chat.tool.expand")}
          </span>
          {stats && (
            <span className={css.resultStats}>
              {t("chat.tool.result_size", {
                lines: stats.lines,
                size: formatBytes(stats.bytes),
              })}
            </span>
          )}
        </button>
      )}

      {!hasBody && !display.head && view.status === "success" && (
        <div className={css.empty}>{t("chat.tool.no_result")}</div>
      )}
    </div>
  );
};

/** 截断规模只报数量：`returned/total` 双值齐全时给区间，否则给单值。 */
function scaleNumbers(
  truncated: NonNullable<ToolCallView["compact"]>["truncated"],
) {
  if (!truncated) return "";
  if (truncated.returned != null && truncated.total != null) {
    return `${truncated.returned}/${truncated.total}`;
  }
  const single = truncated.returned ?? truncated.total;
  return single != null ? String(single) : "";
}
