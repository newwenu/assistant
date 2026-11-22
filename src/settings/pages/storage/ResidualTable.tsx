import React, { useState } from "react";
import { Alert, Empty, Tag, Tooltip } from "antd";
import {
  DownOutlined,
  ExclamationCircleOutlined,
  RightOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import type { I18nPath } from "@/i18n";
import type { ResidualRowView, ResidualViewModel } from "./residualView";
import { tagColor } from "./format";

const RESIDUAL_COLUMN_KEYS = {
  name: "settings.storage.residual.columns.name",
  class: "settings.storage.residual.columns.class",
  scope: "settings.storage.residual.columns.scope",
  scan: "settings.storage.residual.columns.scan",
  declared: "settings.storage.residual.columns.declared",
  residual: "settings.storage.residual.columns.residual",
} as const satisfies Record<string, I18nPath>;

const RESIDUAL_COLUMN_ORDER = [
  "name",
  "class",
  "scope",
  "scan",
  "declared",
  "residual",
] as const;

const RESIDUAL_COLUMN_MIN_PX = {
  name: 220,
  class: 132,
  scope: 104,
  scan: 104,
  declared: 104,
  residual: 112,
} as const satisfies Record<(typeof RESIDUAL_COLUMN_ORDER)[number], number>;

const RESIDUAL_GRID_TEMPLATE = [
  "minmax(0, 1.6fr)",
  `minmax(0, ${RESIDUAL_COLUMN_MIN_PX.class}px)`,
  `minmax(0, ${RESIDUAL_COLUMN_MIN_PX.scope}px)`,
  `minmax(0, ${RESIDUAL_COLUMN_MIN_PX.scan}px)`,
  `minmax(0, ${RESIDUAL_COLUMN_MIN_PX.declared}px)`,
  `minmax(0, ${RESIDUAL_COLUMN_MIN_PX.residual}px)`,
].join(" ");

const RESIDUAL_ROW_GAP_PX = 12;

const RESIDUAL_ROW_PADDING_X_PX = 12;

type ResidualTableVars = React.CSSProperties &
  Record<"--storage-residual-gap" | "--storage-residual-pad-x", string>;

const RESIDUAL_TABLE_VARS: ResidualTableVars = {
  "--storage-residual-gap": `${RESIDUAL_ROW_GAP_PX}px`,
  "--storage-residual-pad-x": `${RESIDUAL_ROW_PADDING_X_PX}px`,
};

interface ResidualTableProps {
  view: ResidualViewModel;
}

function residualChildrenDomId(name: string): string {
  return `storage-residual-children-${name.replace(/[^a-zA-Z0-9_-]/g, "_")}`;
}

export const ResidualTable: React.FC<ResidualTableProps> = ({ view }) => {
  const { t } = useTranslation();
  const [overrides, setOverrides] = useState<Map<string, boolean>>(new Map());

  const isOpen = (row: ResidualRowView): boolean => {
    const override = overrides.get(row.name);
    if (override !== undefined) return override;
    return !row.isDeclared;
  };

  const toggleRow = (row: ResidualRowView): void => {
    const next = !isOpen(row);
    setOverrides((prev) => {
      const map = new Map(prev);
      map.set(row.name, next);
      return map;
    });
  };

  const renderRow = (row: ResidualRowView): React.ReactNode => {
    const open = isOpen(row);
    const childrenId = residualChildrenDomId(row.name);
    const nameNode = row.overRegistered ? (
      <Tooltip
        title={t("settings.storage.residual.notice.over_registered")}
        mouseEnterDelay={0.3}
      >
        <span className="storage-residual-node">{row.name}</span>
      </Tooltip>
    ) : (
      <span className="storage-residual-node">{row.name}</span>
    );

    return (
      <div key={row.name} className="storage-residual-item">
        <div
          className={
            row.overRegistered
              ? "storage-residual-row is-over"
              : "storage-residual-row"
          }
          style={{ gridTemplateColumns: RESIDUAL_GRID_TEMPLATE }}
        >
          <div className="storage-residual-name">
            {row.expandable ? (
              <button
                type="button"
                className="storage-residual-name-toggle"
                aria-expanded={open}
                aria-controls={childrenId}
                onClick={() => toggleRow(row)}
              >
                <span className="storage-residual-expand" aria-hidden="true">
                  {open ? <DownOutlined /> : <RightOutlined />}
                </span>
                {nameNode}
              </button>
            ) : (
              <>
                <span className="storage-residual-expand-spacer" />
                {nameNode}
              </>
            )}
          </div>

          <div className="storage-residual-cell">
            <Tag color={tagColor(row.classTone)}>{row.classText}</Tag>
          </div>

          <div className="storage-residual-cell">{row.scopeText}</div>
          <div className="storage-residual-cell is-num">{row.scanText}</div>
          <div className="storage-residual-cell is-num">{row.declaredText}</div>

          <div className="storage-residual-cell is-num">
            {row.residualText}
            {row.overRegistered && (
              <Tooltip
                title={t("settings.storage.residual.notice.over_registered")}
                mouseEnterDelay={0.3}
              >
                <ExclamationCircleOutlined className="storage-residual-over-icon" />
              </Tooltip>
            )}
          </div>
        </div>

        {row.expandable && (
          <div
            id={childrenId}
            className={
              open
                ? "storage-residual-children-wrap is-open"
                : "storage-residual-children-wrap"
            }
          >
            <div className="storage-residual-children-inner">
              <div className="storage-residual-children">
                {row.children.map((child, index) => (
                  <div
                    key={`${index}-${child.name}`}
                    className="storage-residual-child"
                  >
                    <span className="storage-residual-child-name">
                      {child.name}
                    </span>
                    <span className="storage-residual-child-value is-num">
                      {child.text}
                    </span>
                  </div>
                ))}
                {row.childrenMoreText !== null && (
                  <div className="storage-residual-child is-more">
                    {row.childrenMoreText}
                  </div>
                )}
              </div>
            </div>
          </div>
        )}
      </div>
    );
  };

  return (
    <div className="storage-residual">
      {view.notices.map((notice, index) => (
        <Alert
          key={`${notice.level}-${index}`}
          type={notice.level}
          showIcon
          className="storage-residual-notice"
          title={notice.text}
          description={notice.detail}
        />
      ))}

      {view.empty ? (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={t("settings.storage.residual.notice.empty")}
        />
      ) : (
        <div className="storage-residual-table" style={RESIDUAL_TABLE_VARS}>
          <div
            className="storage-residual-header"
            style={{ gridTemplateColumns: RESIDUAL_GRID_TEMPLATE }}
          >
            {RESIDUAL_COLUMN_ORDER.map((column) => (
              <div
                key={column}
                className={
                  column === "scan" ||
                  column === "declared" ||
                  column === "residual"
                    ? "storage-residual-col is-num"
                    : "storage-residual-col"
                }
              >
                {t(RESIDUAL_COLUMN_KEYS[column])}
              </div>
            ))}
          </div>

          {view.rows.map(renderRow)}

          <div className="storage-residual-totals">
            {view.totalsLines.map((line) => (
              <span key={line.key} className="storage-residual-totals-line">
                {line.text}
              </span>
            ))}
            {view.unnameableText === null ? null : (
              <span className="storage-residual-totals-line is-note">
                {view.unnameableText}
              </span>
            )}
            <span className="storage-residual-totals-hint">
              {view.totalsHint}
            </span>
          </div>
        </div>
      )}
    </div>
  );
};
