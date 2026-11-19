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
import type { ToolCallStatus, ToolCallView } from "../core/chatTypes";
import css from "./ToolCallDocStub.module.css";

interface ToolCallDocStubProps {
  view: ToolCallView;
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
 * 文档批注面的单行引用（I46 §3.7.2）：标题取 `compact.head`，
 * 跳转目标取 `compact.anchor` 的首个行区间。不读结果正文，形态恒为一行。
 */
export const ToolCallDocStub: React.FC<ToolCallDocStubProps> = ({
  view,
  onOpenDocument,
}) => {
  const { t } = useTranslation();
  const anchor = view.compact?.anchor ?? null;
  const title = view.compact?.head ?? view.name;
  const line = anchor?.ranges[0]?.start ?? 1;
  const path = anchor?.path ?? null;

  const open = () => {
    if (path != null) onOpenDocument?.(path, line);
  };

  return (
    <button
      type="button"
      className={`${css.stub} ${statusClass(view.status)}`}
      onClick={open}
      disabled={path == null || onOpenDocument == null}
      title={title}
    >
      <span className={css.statusIcon}>
        <StatusIcon status={view.status} />
      </span>
      <span className={css.head}>{title}</span>
      <span className={css.jump}>{t("chat.tool.doc_jump", { line })}</span>
    </button>
  );
};
