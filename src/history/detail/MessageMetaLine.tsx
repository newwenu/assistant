import React, { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { PushpinOutlined } from "@ant-design/icons";
import type { Message, MessageStatus } from "../../chat/core/chatTypes";
import { parseContent } from "../../chat/core/message/contentBlockUtils";
import css from "./HistoryBranchView.module.css";

interface MessageMetaLineProps {
  message: Message;
  defaultExpanded?: boolean;
}

type StatusLabelKey =
  | "history.detail.status_interrupted"
  | "history.detail.status_failed"
  | "history.detail.status_streaming";

const STATUS_LABEL_KEY: Partial<Record<MessageStatus, StatusLabelKey>> = {
  interrupted: "history.detail.status_interrupted",
  failed: "history.detail.status_failed",
  streaming: "history.detail.status_streaming",
};

function pad(value: number): string {
  return value < 10 ? `0${value}` : String(value);
}

function formatStamp(timestamp: number, withDate: boolean): string {
  const date = new Date(timestamp);
  const time = `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
  if (!withDate) return time;
  const day = `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
  return `${day} ${time}`;
}

export const MessageMetaLine: React.FC<MessageMetaLineProps> = ({
  message,
  defaultExpanded = false,
}) => {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(defaultExpanded);

  const blockCount = useMemo(
    () => parseContent(message.content)?.length ?? 1,
    [message.content],
  );

  const coordKnown = message.turnIndex != null;
  const coord = coordKnown
    ? t("history.detail.coord", {
        turn: message.turnIndex,
        iteration: message.iteration,
      })
    : t("history.detail.coord_unknown");

  const statusKey = message.status
    ? STATUS_LABEL_KEY[message.status]
    : undefined;

  const empty = t("history.detail.empty");

  const fields: Array<[string, string]> = [
    [t("history.detail.fields.id"), message.id],
    [t("history.detail.fields.conversation"), message.conversationId],
    [t("history.detail.fields.role"), message.role],
    [t("history.detail.fields.coord"), coord],
    [
      t("history.detail.fields.parent"),
      message.parentId ?? t("history.detail.root_parent"),
    ],
    [
      t("history.detail.fields.siblings"),
      `${message.siblingIndex} / ${message.siblingCount ?? "?"}`,
    ],
    [t("history.detail.fields.status"), message.status ?? empty],
    [
      t("history.detail.fields.tokens"),
      message.tokens == null ? empty : String(message.tokens),
    ],
    [t("history.detail.fields.provider"), message.providerId ?? empty],
    [t("history.detail.fields.model"), message.modelId ?? empty],
    [t("history.detail.fields.mode"), message.modeId ?? empty],
    [
      t("history.detail.fields.timestamp"),
      formatStamp(message.timestamp, true),
    ],
    [
      t("history.detail.fields.content"),
      `${t("history.detail.chars", { value: message.content.length })} · ${t("history.detail.blocks", { value: blockCount })}`,
    ],
    [
      t("history.detail.fields.reasoning"),
      t("history.detail.chars", { value: message.reasoning?.length ?? 0 }),
    ],
    [
      t("history.detail.fields.tool_calls"),
      t("history.detail.calls", { value: message.toolCalls?.length ?? 0 }),
    ],
  ];

  return (
    <div className={css.metaLine}>
      <button
        type="button"
        className={css.metaChips}
        onClick={() => setExpanded((value) => !value)}
        title={t("history.detail.toggle_details")}
      >
        <span className={coordKnown ? css.metaChip : css.metaChipWarn}>
          {coord}
        </span>
        {message.tokens != null && (
          <span className={css.metaChip}>
            {t("history.detail.tokens", { value: message.tokens })}
          </span>
        )}
        {statusKey && <span className={css.metaChip}>{t(statusKey)}</span>}
        {message.modelId && (
          <span className={css.metaChip}>{message.modelId}</span>
        )}
        {message.providerId && (
          <span className={css.metaChip}>{message.providerId}</span>
        )}
        {message.isPinned && (
          <span className={css.metaChip}>
            <PushpinOutlined />
          </span>
        )}
        <span className={css.metaChipTime}>
          {formatStamp(message.timestamp, false)}
        </span>
      </button>
      {expanded && (
        <dl className={css.metaTable}>
          {fields.map(([label, value]) => (
            <div className={css.metaRow} key={label}>
              <dt className={css.metaLabel}>{label}</dt>
              <dd className={css.metaValue}>{value}</dd>
            </div>
          ))}
        </dl>
      )}
    </div>
  );
};
