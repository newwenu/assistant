import { useEffect, useMemo, useRef, useState } from "react";
import {
  Alert,
  Button,
  Empty,
  Select,
  Space,
  Spin,
  Tag,
  Tooltip,
  Typography,
} from "antd";
import { ReloadOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import {
  permissionService,
  type PermissionAuditEntry,
} from "@/services/permissionService";
import { permissionAuditOutcomeText } from "@/audit/lexicon";
import { chatService } from "@/services/chatService";
import type { Conversation } from "@/chat/core/chatTypes";

const DEFAULT_LIMIT = 50;
const LIMIT_OPTIONS = [50, 100, 300, 500];
const RANGE_KEYS = ["all", "24h", "7d", "30d"] as const;
const COLUMN_KEYS = [
  "time",
  "turn",
  "tool",
  "action",
  "resource",
  "outcome",
] as const;
const LIST_MAX_HEIGHT = 320;
const NO_VALUE = "—";
const COLUMN_MIN_WIDTH = 64;
const COLUMN_MAX_WIDTH = 420;
const COLUMN_GUTTER = 8;
const RESIZE_HANDLE_WIDTH = 8;
const COLUMN_BORDER = "1px solid var(--color-border-secondary)";

const DEFAULT_COLUMN_WIDTHS: Record<ColumnKey, number> = {
  time: 132,
  turn: 168,
  tool: 104,
  action: 104,
  resource: 232,
  outcome: 124,
};

type RangeKey = (typeof RANGE_KEYS)[number];
type ColumnKey = (typeof COLUMN_KEYS)[number];

function clampColumnWidth(width: number): number {
  return Math.min(COLUMN_MAX_WIDTH, Math.max(COLUMN_MIN_WIDTH, width));
}

/**
 * 单元格容器：负责分界线与内边距，让 `grid` 轨道边界即视觉分界。
 * `minWidth: 0` 是省略号生效的前提（默认 `auto` 会撑破轨道）。
 */
function columnCellStyle(isLast: boolean): React.CSSProperties {
  return {
    position: "relative",
    boxSizing: "border-box",
    display: "flex",
    alignItems: "center",
    minWidth: 0,
    paddingInlineStart: COLUMN_GUTTER,
    paddingInlineEnd: isLast ? 0 : COLUMN_GUTTER,
    borderInlineEnd: isLast ? undefined : COLUMN_BORDER,
  };
}

const ELLIPSIS_TEXT_STYLE: React.CSSProperties = {
  minWidth: 0,
  flex: "1 1 auto",
};

const CLIPPED_TAG_STYLE: React.CSSProperties = {
  display: "inline-block",
  marginInlineEnd: 0,
  minWidth: 0,
  maxWidth: "100%",
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
};

const RANGE_HOURS: Record<RangeKey, number | null> = {
  all: null,
  "24h": 24,
  "7d": 168,
  "30d": 720,
};

const ADMIN_TOOLS = new Set(["workspace", "grant"]);
const TOOLBOX_RESOURCE = "toolbox";

const POLICY_OUTCOMES = new Set([
  "allowed",
  "grantHit",
  "denied",
  "approved",
  "sessionApproved",
  "alwaysApproved",
  "rejected",
  "timedOut",
]);
const APPROVED_OUTCOMES = new Set([
  "approved",
  "sessionApproved",
  "alwaysApproved",
]);
const ASKED_OUTCOMES = new Set([...APPROVED_OUTCOMES, "rejected", "timedOut"]);

const OUTCOME_COLORS: Record<string, string> = {
  allowed: "green",
  grantHit: "green",
  approved: "blue",
  sessionApproved: "blue",
  alwaysApproved: "blue",
  denied: "red",
  rejected: "red",
  validateRejected: "red",
  intentError: "red",
  timedOut: "orange",
};

interface AuditStats {
  decisions: number;
  policy: number;
  denied: number;
  asked: number;
  approved: number;
}

function rangeToSince(range: RangeKey): number | null {
  const hours = RANGE_HOURS[range];
  return hours === null ? null : Date.now() - hours * 3_600_000;
}

function isDecisionRow(row: PermissionAuditEntry): boolean {
  return !ADMIN_TOOLS.has(row.tool) && row.resource !== TOOLBOX_RESOURCE;
}

function computeStats(rows: PermissionAuditEntry[]): AuditStats {
  const stats: AuditStats = {
    decisions: 0,
    policy: 0,
    denied: 0,
    asked: 0,
    approved: 0,
  };
  for (const row of rows) {
    if (!isDecisionRow(row)) continue;
    stats.decisions += 1;
    if (!POLICY_OUTCOMES.has(row.outcome)) continue;
    stats.policy += 1;
    if (row.outcome === "denied") stats.denied += 1;
    if (ASKED_OUTCOMES.has(row.outcome)) {
      stats.asked += 1;
      if (APPROVED_OUTCOMES.has(row.outcome)) stats.approved += 1;
    }
  }
  return stats;
}

function percent(numerator: number, denominator: number): string {
  if (denominator === 0) return "—";
  return `${Math.round((numerator / denominator) * 100)}%`;
}

function outcomeColor(outcome: string): string | undefined {
  return Object.prototype.hasOwnProperty.call(OUTCOME_COLORS, outcome)
    ? OUTCOME_COLORS[outcome]
    : undefined;
}

function summarizeResource(resource: string): string {
  const parts = resource.split(/[\\/]+/).filter(Boolean);
  if (parts.length === 0) return resource;
  const base = parts[parts.length - 1];
  return parts.length === 1 ? base : `…/${base}`;
}

function formatTime(millis: number): string {
  return new Date(millis).toLocaleString();
}

/**
 * 权限决策追溯面板（I50 §3.7 / §6）。
 *
 * 与 `PermissionGrantsSection`（当前授权）分离：本面板只读历史决策行，不缓存、
 * 不做增量合并（EF1）。统计只取网关决策行，排除工作区切换、授权管理与 ToolBox
 * 直调行；不得用消息侧 `toolCallView.status` 反推拒绝（OF2）。
 * 结果列表放在定高滚动区内、表头吸顶，避免上百条记录把页面拉长后难以翻看。
 */
export const PermissionAuditSection: React.FC = () => {
  const { t } = useTranslation();
  const [range, setRange] = useState<RangeKey>("all");
  const [limit, setLimit] = useState(DEFAULT_LIMIT);
  const [rows, setRows] = useState<PermissionAuditEntry[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [reloadToken, setReloadToken] = useState(0);
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [page, setPage] = useState(0);
  const [hasMore, setHasMore] = useState(false);
  const [columnWidths, setColumnWidths] = useState(DEFAULT_COLUMN_WIDTHS);
  const [resizing, setResizing] = useState<ColumnKey | null>(null);
  const dragRef = useRef<{
    key: ColumnKey;
    startX: number;
    startWidth: number;
  } | null>(null);
  /**
   * 翻页窗口上界：首页加载时冻结为最新一行的 `atMillis`，后续页都带上它。
   * 否则新写入的决策会插到头部，把后续页整体挤偏（出现重复 / 漏项）。
   */
  const pageAnchorRef = useRef<number | null>(null);

  const resetPaging = () => {
    pageAnchorRef.current = null;
    setPage(0);
  };

  const since = useMemo(() => rangeToSince(range), [range]);

  const gridTemplate = useMemo(
    () =>
      COLUMN_KEYS.map((key, index) =>
        index === COLUMN_KEYS.length - 1
          ? `minmax(${columnWidths[key]}px, 1fr)`
          : `${columnWidths[key]}px`,
      ).join(" "),
    [columnWidths],
  );

  const tableWidth = useMemo(
    () => COLUMN_KEYS.reduce((sum, key) => sum + columnWidths[key], 0),
    [columnWidths],
  );

  useEffect(() => {
    if (resizing === null) return;
    const onMove = (event: PointerEvent) => {
      const drag = dragRef.current;
      if (drag === null) return;
      const next = clampColumnWidth(
        drag.startWidth + (event.clientX - drag.startX),
      );
      setColumnWidths((prev) =>
        prev[drag.key] === next ? prev : { ...prev, [drag.key]: next },
      );
    };
    const onEnd = () => {
      dragRef.current = null;
      setResizing(null);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onEnd);
    window.addEventListener("pointercancel", onEnd);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onEnd);
      window.removeEventListener("pointercancel", onEnd);
    };
  }, [resizing]);

  useEffect(() => {
    if (resizing === null) return;
    const previousCursor = document.body.style.cursor;
    const previousUserSelect = document.body.style.userSelect;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
    return () => {
      document.body.style.cursor = previousCursor;
      document.body.style.userSelect = previousUserSelect;
    };
  }, [resizing]);

  const startResize = (
    key: ColumnKey,
    event: React.PointerEvent<HTMLDivElement>,
  ) => {
    if (event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    dragRef.current = {
      key,
      startX: event.clientX,
      startWidth: columnWidths[key],
    };
    setResizing(key);
  };

  const resetColumnWidth = (key: ColumnKey) => {
    setColumnWidths((prev) =>
      prev[key] === DEFAULT_COLUMN_WIDTHS[key]
        ? prev
        : { ...prev, [key]: DEFAULT_COLUMN_WIDTHS[key] },
    );
  };

  useEffect(() => {
    let cancelled = false;
    void chatService
      .listConversations()
      .then((list) => {
        if (!cancelled) setConversations(list);
      })
      .catch(() => {
        if (!cancelled) setConversations([]);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    const until = page === 0 ? null : pageAnchorRef.current;
    void permissionService
      .listAudit(limit, since, until, page * limit)
      .then((result) => {
        if (cancelled) return;
        if (page === 0) {
          pageAnchorRef.current = result.entries[0]?.atMillis ?? null;
        }
        setRows(result.entries);
        setHasMore(result.hasMore);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setRows([]);
        setHasMore(false);
        setError(err instanceof Error ? err.message : String(err));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [limit, since, page, reloadToken]);

  const stats = useMemo(() => computeStats(rows ?? []), [rows]);

  const conversationTitle = (id: string | null): string => {
    if (!id) return "";
    return conversations.find((c) => c.id === id)?.title || id;
  };

  const turnText = (row: PermissionAuditEntry): string => {
    const label = conversationTitle(row.conversationId);
    if (label === "") return NO_VALUE;
    if (row.iteration === null) return label;
    return `${label} · ${t("settings.agent_mode.audit.turn.iteration_value", {
      count: row.iteration,
    })}`;
  };

  const turnDetail = (row: PermissionAuditEntry): string => {
    const lines = [
      `${t("settings.agent_mode.audit.turn.conversation")}: ${conversationTitle(row.conversationId)}`,
      `${t("settings.agent_mode.audit.turn.conversation_id")}: ${row.conversationId ?? NO_VALUE}`,
      `${t("settings.agent_mode.audit.turn.turn_id")}: ${row.turnId ?? NO_VALUE}`,
      `${t("settings.agent_mode.audit.turn.iteration")}: ${row.iteration ?? NO_VALUE}`,
    ];
    return lines.join("\n");
  };

  const rangeOptions = useMemo(
    () =>
      RANGE_KEYS.map((key) => ({
        value: key,
        label: t(`settings.agent_mode.audit.range.${key}`),
      })),
    [t],
  );

  const visibleRows = rows ?? [];

  return (
    <div style={{ marginTop: 32 }}>
      <Typography.Title level={5} style={{ margin: 0 }}>
        {t("settings.agent_mode.audit.title")}
      </Typography.Title>
      <Typography.Text
        type="secondary"
        style={{ fontSize: 12, display: "block", marginBottom: 8 }}
      >
        {t("settings.agent_mode.audit.description")}
      </Typography.Text>

      <Space wrap style={{ marginBottom: 8 }}>
        <Select<RangeKey>
          style={{ width: 140 }}
          value={range}
          onChange={(value) => {
            setRange(value);
            resetPaging();
          }}
          options={rangeOptions}
        />
        <Select<number>
          style={{ width: 120 }}
          value={limit}
          onChange={(value) => {
            setLimit(value);
            resetPaging();
          }}
          options={LIMIT_OPTIONS.map((value) => ({
            value,
            label: t("settings.agent_mode.audit.limit_value", { count: value }),
          }))}
        />
        <Button
          icon={<ReloadOutlined />}
          loading={loading}
          onClick={() => {
            resetPaging();
            setReloadToken((token) => token + 1);
          }}
        >
          {t("settings.agent_mode.audit.refresh")}
        </Button>
      </Space>

      <Space size={16} wrap style={{ marginBottom: 4 }}>
        <Typography.Text style={{ fontSize: 12 }}>
          {t("settings.agent_mode.audit.stats.decisions", {
            count: stats.decisions,
          })}
        </Typography.Text>
        <Tooltip
          title={t("settings.agent_mode.audit.stats.deny_detail", {
            denied: stats.denied,
            total: stats.policy,
          })}
        >
          <Typography.Text style={{ fontSize: 12 }}>
            {`${t("settings.agent_mode.audit.stats.deny_rate")} ${percent(stats.denied, stats.policy)}`}
          </Typography.Text>
        </Tooltip>
        <Tooltip
          title={t("settings.agent_mode.audit.stats.approve_detail", {
            approved: stats.approved,
            asked: stats.asked,
          })}
        >
          <Typography.Text style={{ fontSize: 12 }}>
            {`${t("settings.agent_mode.audit.stats.approve_rate")} ${percent(stats.approved, stats.asked)}`}
          </Typography.Text>
        </Tooltip>
      </Space>
      <Typography.Text
        type="secondary"
        style={{ fontSize: 12, display: "block", marginBottom: 8 }}
      >
        {t("settings.agent_mode.audit.stats.excluded_hint")}
      </Typography.Text>

      {error !== null && (
        <Alert
          type="error"
          showIcon
          style={{ marginBottom: 12 }}
          title={t("settings.agent_mode.audit.load_failed")}
          description={error}
        />
      )}

      {rows !== null && (
        <Space size={8} wrap style={{ marginBottom: 4 }}>
          <Typography.Text type="secondary" style={{ fontSize: 12 }}>
            {t("settings.agent_mode.audit.rows_count", {
              count: visibleRows.length,
            })}
          </Typography.Text>
          <Button
            size="small"
            disabled={page === 0 || loading}
            onClick={() => setPage((prev) => Math.max(0, prev - 1))}
          >
            {t("settings.agent_mode.audit.page_prev")}
          </Button>
          <Typography.Text type="secondary" style={{ fontSize: 12 }}>
            {t("settings.agent_mode.audit.page_info", { page: page + 1 })}
          </Typography.Text>
          <Button
            size="small"
            disabled={!hasMore || loading}
            onClick={() => setPage((prev) => prev + 1)}
          >
            {t("settings.agent_mode.audit.page_next")}
          </Button>
          {hasMore && (
            <Typography.Text type="secondary" style={{ fontSize: 12 }}>
              {t("settings.agent_mode.audit.more_hint")}
            </Typography.Text>
          )}
        </Space>
      )}

      {loading && rows === null ? (
        <div style={{ display: "flex", justifyContent: "center", padding: 24 }}>
          <Spin />
        </div>
      ) : visibleRows.length === 0 ? (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={t("settings.agent_mode.audit.empty")}
        />
      ) : (
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
                        aria-label={t(
                          "settings.agent_mode.audit.column_resize",
                        )}
                        onPointerDown={(event) => startResize(key, event)}
                        onDoubleClick={() => resetColumnWidth(key)}
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
          {visibleRows.map((row, index) => (
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
                <Tag
                  color={outcomeColor(row.outcome)}
                  style={CLIPPED_TAG_STYLE}
                >
                  {permissionAuditOutcomeText(row.outcome)}
                </Tag>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};
