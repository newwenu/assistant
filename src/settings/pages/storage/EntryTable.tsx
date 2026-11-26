import React, { useMemo, useState } from "react";
import { Button, Tag, Tooltip } from "antd";
import {
  DownOutlined,
  InfoCircleOutlined,
  RightOutlined,
  SafetyOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import type {
  StorageEntryDto,
  StorageReclaimActionDto,
} from "@/types/protocol.contract";
import {
  ENTRY_COLUMN_LABEL_KEYS,
  ENTRY_COLUMN_ORDER,
  ENTRY_GRID_TEMPLATE,
  ENTRY_ROW_GAP_PX,
  ENTRY_ROW_PADDING_X_PX,
  buildBlocks,
} from "./entryColumns";
import { entryDetailView, type EntryRowView } from "./entryView";
import { bytesOrDash, countText } from "./format";
import { groupNameText } from "./lexicon";
import { EntryDetail } from "./EntryDetail";

interface EntryTableProps {
  rows: EntryRowView[];
  entries: StorageEntryDto[];
  expandedKey: string | null;
  cleanupBusy: boolean;
  onToggleDetail: (key: string) => void;
  onCleanup: (key: string, action: StorageReclaimActionDto) => void;
}

function detailDomId(key: string): string {
  return `storage-entry-detail-${key.replace(/[^a-zA-Z0-9_-]/g, "_")}`;
}

type EntryTableVars = React.CSSProperties &
  Record<"--storage-entry-gap" | "--storage-entry-pad-x", string>;

const ENTRY_TABLE_VARS: EntryTableVars = {
  "--storage-entry-gap": `${ENTRY_ROW_GAP_PX}px`,
  "--storage-entry-pad-x": `${ENTRY_ROW_PADDING_X_PX}px`,
};

export const EntryTable: React.FC<EntryTableProps> = ({
  rows,
  entries,
  expandedKey,
  cleanupBusy,
  onToggleDetail,
  onCleanup,
}) => {
  const { t } = useTranslation();
  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(
    new Set(),
  );

  const blocks = useMemo(() => buildBlocks(rows), [rows]);

  const entryByKey = useMemo((): Map<string, StorageEntryDto> => {
    const map = new Map<string, StorageEntryDto>();
    for (const entry of entries) map.set(entry.key, entry);
    return map;
  }, [entries]);

  const toggleGroup = (group: string): void => {
    setCollapsedGroups((prev) => {
      const next = new Set(prev);
      if (next.has(group)) next.delete(group);
      else next.add(group);
      return next;
    });
  };

  const renderCleanup = (
    row: EntryRowView,
    entry: StorageEntryDto,
  ): React.ReactNode => {
    const view = row.cleanup;
    if (view.kind === "none") {
      return <span className="storage-entry-dash">{bytesOrDash(null)}</span>;
    }
    if (view.kind === "text") {
      return <span className="storage-entry-owner">{view.text}</span>;
    }

    const button = (
      <Button
        type="text"
        size="small"
        danger={!view.disabled}
        disabled={view.disabled || cleanupBusy}
        icon={view.strongConfirm ? <SafetyOutlined /> : undefined}
        onClick={() => onCleanup(row.key, entry.reclaim.action)}
      >
        {t("settings.storage.entries.cleanup_button")}
      </Button>
    );

    if (view.tooltip === null) return button;
    return (
      <Tooltip title={view.tooltip} mouseEnterDelay={0.4}>
        <span className="storage-entry-tooltip-host">{button}</span>
      </Tooltip>
    );
  };

  const renderRow = (row: EntryRowView): React.ReactNode => {
    const entry = entryByKey.get(row.key);
    const expanded = expandedKey === row.key;
    const detailId = detailDomId(row.key);

    return (
      <div key={row.key} className="storage-entry-item">
        <div className="storage-entry-row">
          <button
            type="button"
            className="storage-entry-name"
            aria-expanded={expanded}
            aria-controls={detailId}
            onClick={() => onToggleDetail(row.key)}
          >
            <span className="storage-entry-expand" aria-hidden="true">
              {expanded ? <DownOutlined /> : <RightOutlined />}
            </span>
            <span className="storage-entry-title">{row.name}</span>
          </button>

          <div className="storage-entry-cell is-num">
            <span className="storage-entry-size-line">
              <span className="storage-entry-size">{row.sizeText}</span>
              {row.availabilityBadge !== null && (
                <Tooltip
                  title={row.availabilityBadge.reason}
                  mouseEnterDelay={0.4}
                >
                  <InfoCircleOutlined
                    className="storage-entry-reason-icon"
                    style={{ color: badgeColor(row.availabilityBadge.tone) }}
                  />
                </Tooltip>
              )}
            </span>
            {row.countText !== null && (
              <span className="storage-entry-meta">{row.countText}</span>
            )}
          </div>

          <div className="storage-entry-cell">
            {row.policyChips.length > 2 ? (
              <Tooltip
                title={row.policyChips.join(" · ")}
                mouseEnterDelay={0.4}
              >
                <span className="storage-entry-chips">
                  {row.policyChips.slice(0, 2).map((chip) => (
                    <Tag key={chip}>{chip}</Tag>
                  ))}
                  <Tag>+{row.policyChips.length - 2}</Tag>
                </span>
              </Tooltip>
            ) : (
              <span className="storage-entry-chips">
                {row.policyChips.map((chip) => (
                  <Tag key={chip}>{chip}</Tag>
                ))}
              </span>
            )}
          </div>

          <div className="storage-entry-cleanup">
            {entry === undefined ? (
              <span className="storage-entry-dash">{bytesOrDash(null)}</span>
            ) : (
              renderCleanup(row, entry)
            )}
          </div>
        </div>

        {entry !== undefined && (
          <div
            className={
              expanded
                ? "storage-entry-detail-wrap is-open"
                : "storage-entry-detail-wrap"
            }
          >
            <div className="storage-entry-detail-inner">
              <EntryDetail id={detailId} detail={entryDetailView(entry)} />
            </div>
          </div>
        )}
      </div>
    );
  };

  return (
    <div
      className="storage-entry-table"
      style={{
        ...ENTRY_TABLE_VARS,
        gridTemplateColumns: ENTRY_GRID_TEMPLATE,
      }}
    >
      <div className="storage-entry-header">
        {ENTRY_COLUMN_ORDER.map((column) => (
          <div
            key={column}
            className={
              column === "size"
                ? "storage-entry-col is-num"
                : "storage-entry-col"
            }
          >
            {t(ENTRY_COLUMN_LABEL_KEYS[column])}
          </div>
        ))}
      </div>

      {blocks.map((block) => {
        const group = block.group;
        if (group === null) {
          return renderRow(block.rows[0]);
        }

        const collapsed = collapsedGroups.has(group);
        return (
          <div key={group} className="storage-entry-group-wrap">
            <div
              className="storage-entry-group"
              onClick={() => toggleGroup(group)}
            >
              <span className="storage-entry-group-arrow">
                {collapsed ? <RightOutlined /> : <DownOutlined />}
              </span>
              <span className="storage-entry-group-label">
                {groupNameText(group)}
              </span>
              <span className="storage-entry-group-count">
                {countText(block.rows.length)}
              </span>
            </div>

            {!collapsed && block.rows.map(renderRow)}
          </div>
        );
      })}
    </div>
  );
};

function badgeColor(
  tone: "neutral" | "success" | "warning" | "error" | "info",
) {
  switch (tone) {
    case "warning":
      return "var(--ant-color-warning)";
    case "error":
      return "var(--ant-color-error)";
    default:
      return "var(--color-text-tertiary)";
  }
}
