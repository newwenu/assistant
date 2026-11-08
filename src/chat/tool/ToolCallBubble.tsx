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
  formatToolDuration,
  parsePersistedOutput,
  resultStats,
  shouldCollapseResult,
  summarizeArguments,
} from "./toolCallDisplay";
import css from "./ToolCallBubble.module.css";

interface ToolCallBubbleProps {
  view: ToolCallView;
}

function StatusIcon({ status }: { status: ToolCallStatus }) {
  switch (status) {
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

  const persisted = parsePersistedOutput(view.resultText);
  const resultText = persisted ? persisted.preview : (view.resultText ?? "");
  const hasResult = resultText.trim().length > 0;
  const collapsible = hasResult && shouldCollapseResult(resultText);
  const showResult = hasResult && (expanded || !collapsible);
  const argsSummary =
    summarizeArguments(view.arguments) ??
    (view.argsText?.trim() ? view.argsText.trim() : null);
  const duration = formatToolDuration(view.startedAt, view.finishedAt);
  const stats = collapsible ? resultStats(resultText) : null;

  return (
    <div className={`${css.root} ${statusClass(view.status)}`}>
      <div className={css.header}>
        <span className={css.statusIcon}>
          <StatusIcon status={view.status} />
        </span>
        <span className={css.name}>{view.name}</span>
        <span className={css.statusLabel}>
          {t(`chat.tool.status.${view.status}`)}
        </span>
        {duration && <span className={css.duration}>{duration}</span>}
      </div>

      {argsSummary && <div className={css.args}>{argsSummary}</div>}

      {view.status === "awaiting_user" && view.approval && (
        <ApprovalCard approval={view.approval} startedAt={view.startedAt} />
      )}

      {view.status === "awaiting_user" && view.question && (
        <QuestionCard question={view.question} startedAt={view.startedAt} />
      )}

      {persisted && (
        <div className={css.persisted}>
          {t("chat.tool.result_persisted")}
          <span className={css.persistedPath}>{persisted.path}</span>
        </div>
      )}

      {showResult && <pre className={css.result}>{resultText}</pre>}

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

      {!hasResult && view.status === "success" && (
        <div className={css.empty}>{t("chat.tool.no_result")}</div>
      )}
    </div>
  );
};
