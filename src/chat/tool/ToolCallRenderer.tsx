import React from "react";
import {
  CheckCircleOutlined,
  CloseCircleOutlined,
  ExclamationCircleOutlined,
  LoadingOutlined,
  QuestionCircleOutlined,
  StopOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import type { ToolCallView, ToolCallStatus } from "../core/chatTypes";
import {
  formatToolTiming,
  shouldRenderDocStub,
  summarizeArguments,
} from "./toolCallDisplay";
import { useNowTick } from "./useNowTick";
import { ToolCallBubble } from "./ToolCallBubble";
import { ToolCallDocStub } from "./ToolCallDocStub";
import css from "./ToolCallRenderer.module.css";

interface ToolCallRendererProps {
  view: ToolCallView;
  density: "single" | "card";
  /** 文档面板当前打开的文件路径；`link === "chat"` 时恒为 `null`，恒走卡片（I46 §3.7.4）。 */
  docPanelPath?: string | null;
  onOpenDocument?: (path: string, line: number) => void;
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

/**
 * 工具渲染唯一入口（I46 compact §3.7.4）：选路只看「锚点是否命中当前文档」与
 * `density`，**不读** `role` / `modeId` / `TurnKind` / 工具名（门禁 13）。
 * 单行档与批注引用都只取 `compact.head` / 状态 / 耗时，不解析结果正文（门禁 7）。
 */
export const ToolCallRenderer: React.FC<ToolCallRendererProps> = ({
  view,
  density,
  docPanelPath = null,
  onOpenDocument,
}) => {
  const { t } = useTranslation();
  const active = view.status === "preparing" || view.status === "running";
  const now = useNowTick(active);

  if (shouldRenderDocStub(view, docPanelPath)) {
    return <ToolCallDocStub view={view} onOpenDocument={onOpenDocument} />;
  }

  if (density === "card") {
    return <ToolCallBubble view={view} />;
  }

  const argsSummary =
    view.argsPreview?.trim() || summarizeArguments(view.arguments) || null;
  const duration = formatToolTiming(view, now);
  const head = view.compact?.head ?? null;

  return (
    <div className={`${css.row} ${statusClass(view.status)}`}>
      <span className={css.statusIcon}>
        <StatusIcon status={view.status} />
      </span>
      {head ? (
        <span className={css.args}>{head}</span>
      ) : (
        <>
          <span className={css.name}>{view.name}</span>
          {argsSummary && <span className={css.args}>{argsSummary}</span>}
        </>
      )}
      {duration && <span className={css.duration}>{duration}</span>}
      <span className={css.statusLabel}>
        {t(`chat.tool.status.${view.status}`)}
      </span>
    </div>
  );
};
