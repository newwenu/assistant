import { useEffect, useMemo, useRef, useState } from "react";
import {
  Alert,
  Button,
  Empty,
  Select,
  Space,
  Spin,
  Tooltip,
  Typography,
} from "antd";
import { ReloadOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import {
  permissionService,
  type PermissionAuditEntry,
} from "@/services/tool/permissionService";
import { chatService } from "@/services/chat/chatService";
import type { Conversation } from "@/chat/core/chatTypes";

import { AuditTable } from "./AuditTable";
import { COLUMN_KEYS } from "./columns";
import { NO_VALUE } from "./format";
import {
  DEFAULT_LIMIT,
  LIMIT_OPTIONS,
  RANGE_KEYS,
  rangeToSince,
  type RangeKey,
} from "./range";
import { computeStats, percent } from "./stats";
import { useColumnResize } from "./useColumnResize";

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
  const { columnWidths, resizing, startResize, resetColumnWidth } =
    useColumnResize();
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
        <AuditTable
          rows={visibleRows}
          resizing={resizing}
          gridTemplate={gridTemplate}
          tableWidth={tableWidth}
          onStartResize={startResize}
          onResetColumnWidth={resetColumnWidth}
          turnText={turnText}
          turnDetail={turnDetail}
        />
      )}
    </div>
  );
};
