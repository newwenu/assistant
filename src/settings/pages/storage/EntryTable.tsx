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
  ENTRY_DOMAIN_LABEL_KEYS,
  ENTRY_DOMAIN_ORDER,
  ENTRY_GRID_TEMPLATE,
  ENTRY_ROW_GAP_PX,
  ENTRY_ROW_PADDING_X_PX,
  ENTRY_TABLE_MIN_WIDTH,
  domainOfKey,
  type EntryDomain,
} from "./entryColumns";
import { entryDetailView, type EntryRowView } from "./entryView";
import { bytesOrDash, countText, tagColor } from "./format";
import { EntryDetail } from "./EntryDetail";

interface EntryGroup {
  domain: EntryDomain;
  label: string;
  rows: EntryRowView[];
}

interface EntryTableProps {
  rows: EntryRowView[];
  entries: StorageEntryDto[];
  expandedKey: string | null;
  reclaimBusy: boolean;
  onToggleDetail: (key: string) => void;
  onReclaim: (key: string, action: StorageReclaimActionDto) => void;
}

function detailDomId(key: string): string {
  return `storage-entry-detail-${key.replace(/[^a-zA-Z0-9_-]/g, "_")}`;
}

type EntryTableVars = React.CSSProperties &
  Record<
    | "--storage-entry-min-width"
    | "--storage-entry-gap"
    | "--storage-entry-pad-x",
    string
  >;

const ENTRY_TABLE_VARS: EntryTableVars = {
  "--storage-entry-min-width": `${ENTRY_TABLE_MIN_WIDTH}px`,
  "--storage-entry-gap": `${ENTRY_ROW_GAP_PX}px`,
  "--storage-entry-pad-x": `${ENTRY_ROW_PADDING_X_PX}px`,
};

export const EntryTable: React.FC<EntryTableProps> = ({
  rows,
  entries,
  expandedKey,
  reclaimBusy,
  onToggleDetail,
  onReclaim,
}) => {
  const { t } = useTranslation();
  const [collapsedDomains, setCollapsedDomains] = useState<Set<EntryDomain>>(
    new Set(),
  );

  const groups = useMemo((): EntryGroup[] => {
    const buckets = new Map<EntryDomain, EntryRowView[]>();
    for (const row of rows) {
      const domain = domainOfKey(row.key);
      const list = buckets.get(domain) ?? [];
      list.push(row);
      buckets.set(domain, list);
    }

    const result: EntryGroup[] = [];
    for (const domain of ENTRY_DOMAIN_ORDER) {
      const list = buckets.get(domain);
      if (list !== undefined && list.length > 0) {
        result.push({
          domain,
          label: t(ENTRY_DOMAIN_LABEL_KEYS[domain]),
          rows: list,
        });
      }
    }
    const other = buckets.get("other");
    if (other !== undefined) {
      result.push({
        domain: "other",
        label: t(ENTRY_DOMAIN_LABEL_KEYS.other),
        rows: other,
      });
    }
    return result;
  }, [rows, t]);

  const entryByKey = useMemo((): Map<string, StorageEntryDto> => {
    const map = new Map<string, StorageEntryDto>();
    for (const entry of entries) map.set(entry.key, entry);
    return map;
  }, [entries]);

  const toggleDomain = (domain: EntryDomain): void => {
    setCollapsedDomains((prev) => {
      const next = new Set(prev);
      if (next.has(domain)) next.delete(domain);
      else next.add(domain);
      return next;
    });
  };

  const renderAction = (
    row: EntryRowView,
    action: StorageReclaimActionDto,
  ): React.ReactNode => {
    const view = row.action;
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
        disabled={view.disabled || reclaimBusy}
        icon={view.strongConfirm ? <SafetyOutlined /> : undefined}
        onClick={() => onReclaim(row.key, action)}
      >
        {t("settings.storage.entries.reclaim_button")}
      </Button>
    );

    if (view.tooltip === null) return button;
    return (
      <Tooltip title={view.tooltip} mouseEnterDelay={0.4}>
        <span className="storage-entry-tooltip-host">{button}</span>
      </Tooltip>
    );
  };

  return (
    <div className="storage-entry-table" style={ENTRY_TABLE_VARS}>
      <div
        className="storage-entry-header"
        style={{ gridTemplateColumns: ENTRY_GRID_TEMPLATE }}
      >
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

      {groups.map((group) => {
        const collapsed = collapsedDomains.has(group.domain);
        return (
          <div key={group.domain} className="storage-entry-group-wrap">
            <div
              className="storage-entry-group"
              onClick={() => toggleDomain(group.domain)}
            >
              <span className="storage-entry-group-arrow">
                {collapsed ? <RightOutlined /> : <DownOutlined />}
              </span>
              <span className="storage-entry-group-label">{group.label}</span>
              <span className="storage-entry-group-count">
                {countText(group.rows.length)}
              </span>
            </div>

            {!collapsed &&
              group.rows.map((row) => {
                const entry = entryByKey.get(row.key);
                const expanded = expandedKey === row.key;
                const detailId = detailDomId(row.key);
                return (
                  <div key={row.key} className="storage-entry-item">
                    <div
                      className="storage-entry-row"
                      style={{ gridTemplateColumns: ENTRY_GRID_TEMPLATE }}
                    >
                      <div className="storage-entry-name">
                        <div className="storage-entry-name-main">
                          <span className="storage-entry-title">
                            {row.name}
                          </span>
                          <Tooltip title={row.key} mouseEnterDelay={0.6}>
                            <span className="storage-entry-key">{row.key}</span>
                          </Tooltip>
                        </div>
                        <Tooltip
                          title={
                            expanded
                              ? t("settings.storage.entries.hide_details")
                              : t("settings.storage.entries.details")
                          }
                        >
                          <button
                            type="button"
                            className="storage-entry-expand"
                            aria-expanded={expanded}
                            aria-controls={detailId}
                            onClick={() => onToggleDetail(row.key)}
                          >
                            {expanded ? <DownOutlined /> : <RightOutlined />}
                          </button>
                        </Tooltip>
                      </div>

                      <div className="storage-entry-cell is-num">
                        <span className="storage-entry-size">
                          {row.sizeText}
                        </span>
                        {row.countText !== null && (
                          <span className="storage-entry-meta">
                            {row.countText}
                          </span>
                        )}
                      </div>

                      <div className="storage-entry-cell">
                        <Tag color={tagColor(row.availability.tone)}>
                          {row.availability.text}
                        </Tag>
                        {row.availability.reason !== null && (
                          <Tooltip
                            title={row.availability.reason}
                            mouseEnterDelay={0.4}
                          >
                            <InfoCircleOutlined className="storage-entry-reason-icon" />
                          </Tooltip>
                        )}
                      </div>

                      <div className="storage-entry-cell">
                        <Tag color={tagColor(row.eligibility.tone)}>
                          {row.eligibility.text}
                        </Tag>
                        {row.eligibility.secondLine !== null && (
                          <span className="storage-entry-reason">
                            {row.eligibility.secondLine}
                          </span>
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

                      <div className="storage-entry-action">
                        {entry === undefined
                          ? bytesOrDash(null)
                          : renderAction(row, entry.reclaim.action)}
                      </div>
                    </div>

                    {expanded && entry !== undefined && (
                      <EntryDetail
                        id={detailId}
                        detail={entryDetailView(entry)}
                      />
                    )}
                  </div>
                );
              })}
          </div>
        );
      })}
    </div>
  );
};
