import { Tag, Tooltip, Typography } from "antd";
import { useTranslation } from "react-i18next";
import { permissionAuditOutcomeText } from "./permissionOutcomes";
import type { PermissionAuditEntry } from "@/services/tool/permissionService";

import {
  CLIPPED_TAG_STYLE,
  COLUMN_KEYS,
  ELLIPSIS_TEXT_STYLE,
  LIST_MAX_HEIGHT,
  RESIZE_HANDLE_WIDTH,
  columnCellStyle,
  type ColumnKey,
} from "./columns";
import { formatTime, summarizeResource } from "./format";
import { outcomeColor } from "./stats";

interface AuditTableProps {
  rows: PermissionAuditEntry[];
  resizing: ColumnKey | null;
  gridTemplate: string;
  tableWidth: number;
  onStartResize: (
    key: ColumnKey,
    event: React.PointerEvent<HTMLDivElement>,
  ) => void;
  onResetColumnWidth: (key: ColumnKey) => void;
  turnText: (row: PermissionAuditEntry) => string;
  turnDetail: (row: PermissionAuditEntry) => string;
}

/**
 * 定高滚动表格：表头吸顶，行高与表头共用同一 `grid` 轨道模板，
 * 因此列宽拖拽同时作用于表头与数据行。
 */
export const AuditTable: React.FC<AuditTableProps> = ({
  rows,
  resizing,
  gridTemplate,
  tableWidth,
  onStartResize,
  onResetColumnWidth,
  turnText,
  turnDetail,
}) => {
  const { t } = useTranslation();

  return (
    <div
      style={{
        maxHeight: LIST_MAX_HEIGHT,
        overflow: "auto",
        borderTop: "1px solid var(--color-border-secondary)",
      }}
    >
      <div
        style={{
          position: "sticky",
          top: 0,
          zIndex: 1,
          minWidth: tableWidth,
          display: "grid",
          gridTemplateColumns: gridTemplate,
          alignItems: "center",
          padding: "4px 0",
          background: "var(--color-bg-elevated)",
          borderBottom: "1px solid var(--color-border-secondary)",
        }}
      >
        {COLUMN_KEYS.map((key, index) => {
          const isLast = index === COLUMN_KEYS.length - 1;
          return (
            <div key={key} style={columnCellStyle(isLast)}>
              <Typography.Text
                type="secondary"
                ellipsis
                style={{ ...ELLIPSIS_TEXT_STYLE, fontSize: 11 }}
              >
                {t(`settings.agent_mode.audit.columns.${key}`)}
              </Typography.Text>
              {!isLast && (
                <Tooltip
                  title={t("settings.agent_mode.audit.column_resize")}
                  mouseEnterDelay={0.6}
                  open={resizing === null ? undefined : false}
                >
                  <div
                    role="separator"
                    aria-orientation="vertical"
                    aria-label={t("settings.agent_mode.audit.column_resize")}
                    onPointerDown={(event) => onStartResize(key, event)}
                    onDoubleClick={() => onResetColumnWidth(key)}
                    style={{
                      position: "absolute",
                      top: 0,
                      bottom: 0,
                      right: -RESIZE_HANDLE_WIDTH / 2,
                      width: RESIZE_HANDLE_WIDTH,
                      zIndex: 2,
                      cursor: "col-resize",
                      background:
                        resizing === key
                          ? "var(--color-primary)"
                          : "transparent",
                    }}
                  />
                </Tooltip>
              )}
            </div>
          );
        })}
      </div>
      {rows.map((row, index) => (
        <div
          key={`${row.atMillis}-${index}`}
          style={{
            minWidth: tableWidth,
            display: "grid",
            gridTemplateColumns: gridTemplate,
            alignItems: "center",
            padding: "6px 0",
            borderBottom: "1px solid var(--color-border-secondary)",
          }}
        >
          <div style={columnCellStyle(false)}>
            <Typography.Text
              type="secondary"
              ellipsis
              style={{ ...ELLIPSIS_TEXT_STYLE, fontSize: 12 }}
            >
              {formatTime(row.atMillis)}
            </Typography.Text>
          </div>
          <div style={columnCellStyle(false)}>
            <Tooltip
              title={
                <span style={{ whiteSpace: "pre-line" }}>
                  {row.conversationId === null
                    ? t("settings.agent_mode.audit.turn.unattributed_hint")
                    : turnDetail(row)}
                </span>
              }
            >
              <Typography.Text
                type="secondary"
                ellipsis
                style={{ ...ELLIPSIS_TEXT_STYLE, fontSize: 12 }}
              >
                {turnText(row)}
              </Typography.Text>
            </Tooltip>
          </div>
          <div style={columnCellStyle(false)}>
            <Tag style={CLIPPED_TAG_STYLE}>{row.tool}</Tag>
          </div>
          <div style={columnCellStyle(false)}>
            <Typography.Text
              style={{
                ...ELLIPSIS_TEXT_STYLE,
                fontFamily: "var(--font-mono)",
                fontSize: 12,
              }}
            >
              {row.action}
            </Typography.Text>
          </div>
          <div style={columnCellStyle(false)}>
            <Typography.Text
              type="secondary"
              ellipsis
              style={{
                ...ELLIPSIS_TEXT_STYLE,
                fontFamily: "var(--font-mono)",
                fontSize: 12,
              }}
            >
              {summarizeResource(row.resource)}
            </Typography.Text>
          </div>
          <div style={columnCellStyle(true)}>
            <Tag color={outcomeColor(row.outcome)} style={CLIPPED_TAG_STYLE}>
              {permissionAuditOutcomeText(row.outcome)}
            </Tag>
          </div>
        </div>
      ))}
    </div>
  );
};
