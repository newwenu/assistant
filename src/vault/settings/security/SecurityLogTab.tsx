import { useEffect, useMemo, useState } from "react";
import {
  Alert,
  Button,
  Empty,
  Select,
  Space,
  Spin,
  Tag,
  Typography,
} from "antd";
import { ReloadOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { VAULT_AUDIT_EVENT_TYPES, vaultAuditEventText } from "@/audit/lexicon";
import { parseAuditDetail } from "@/audit/detail";
import {
  vaultErrorCode,
  vaultErrorMessage,
} from "../../core/display/vaultError";
import { useVaultShallow } from "../../core/vaultStore";
import type { AuditLogEntry } from "../../core/vault.types";
import styles from "../../vault.module.css";

const DEFAULT_LIMIT = 100;
const LIMIT_OPTIONS = [100, 300, 500];
const RANGE_KEYS = ["all", "24h", "7d", "30d"] as const;

type RangeKey = (typeof RANGE_KEYS)[number];

const RANGE_HOURS: Record<RangeKey, number | null> = {
  all: null,
  "24h": 24,
  "7d": 168,
  "30d": 720,
};

const BACKFILL_PREFIX = "jsonl-";
const UNKNOWN_ENTRY_ID = "unknown";

function formatTime(millis: number): string {
  return new Date(millis).toLocaleString();
}

function rangeToSince(range: RangeKey): number | undefined {
  const hours = RANGE_HOURS[range];
  return hours === null ? undefined : Date.now() - hours * 3_600_000;
}

function isBackfilled(id: string): boolean {
  return id.startsWith(BACKFILL_PREFIX);
}

function openableEntryId(entryId: string | null): string | null {
  if (entryId === null || entryId.length === 0) return null;
  if (entryId === UNKNOWN_ENTRY_ID) return null;
  return entryId;
}

interface SecurityLogTabProps {
  onOpenEntry?: (entryId: string) => void;
}

export default function SecurityLogTab({ onOpenEntry }: SecurityLogTabProps) {
  const { t } = useTranslation();
  const { isLocked, queryAuditLog } = useVaultShallow((s) => ({
    isLocked: s.isLocked,
    queryAuditLog: s.queryAuditLog,
  }));
  const [eventTypes, setEventTypes] = useState<string[]>([]);
  const [range, setRange] = useState<RangeKey>("all");
  const [limit, setLimit] = useState(DEFAULT_LIMIT);
  const [rows, setRows] = useState<AuditLogEntry[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<{
    code: string | null;
    text: string;
  } | null>(null);
  const [reloadToken, setReloadToken] = useState(0);

  const since = useMemo(() => rangeToSince(range), [range]);

  useEffect(() => {
    if (isLocked) {
      setRows(null);
      setError(null);
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);
    void queryAuditLog(limit, since, eventTypes)
      .then((list) => {
        if (!cancelled) setRows(list);
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        setRows([]);
        setError({ code: vaultErrorCode(e), text: vaultErrorMessage(e) });
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [isLocked, queryAuditLog, limit, since, eventTypes, reloadToken]);

  const eventOptions = VAULT_AUDIT_EVENT_TYPES.map((event) => ({
    value: event,
    label: vaultAuditEventText(event),
  }));

  const entryIdText = (entryId: string | null): string =>
    entryId ?? t("vault.security_log.detail.unknown_entry");

  const detailText = (detail: string | null): string => {
    const parsed = parseAuditDetail(detail);
    switch (parsed.kind) {
      case "empty":
        return t("vault.security_log.no_detail");
      case "count":
        return t("vault.security_log.detail.count", { count: parsed.count });
      case "entryId":
        return t("vault.security_log.detail.entry", {
          entryId: parsed.entryId,
        });
      case "entryWithSuffix":
        return t("vault.security_log.detail.entry_with_suffix", {
          entryId: entryIdText(parsed.entryId),
          suffix: parsed.suffix,
        });
      case "taggedEntry":
        return t("vault.security_log.detail.tagged_entry", {
          tag: parsed.tag,
          entryId: entryIdText(parsed.entryId),
        });
      case "entryCount":
        return t("vault.security_log.detail.entry_count", {
          entryId: entryIdText(parsed.entryId),
          count: parsed.count,
        });
      case "export":
        return t(
          parsed.failed
            ? "vault.security_log.detail.export_failed"
            : "vault.security_log.detail.export",
          {
            format: parsed.format,
            count: parsed.count,
            digest: parsed.digest,
          },
        );
      case "dekRotated":
        return t("vault.security_log.detail.dek_rotated", {
          old: parsed.oldVersion,
          new: parsed.newVersion,
          count: parsed.count,
        });
      case "conflict":
        return t("vault.security_log.detail.conflict", {
          entryId: entryIdText(parsed.entryId),
          side: parsed.side,
          field: parsed.field,
          digest: parsed.digest,
        });
      case "fingerprint":
        return t("vault.security_log.detail.fingerprint", {
          keyType: parsed.keyType,
          digest: parsed.digest,
        });
      case "label":
        return parsed.label;
      case "raw":
        return parsed.raw;
    }
  };

  const rangeOptions = useMemo(
    () =>
      RANGE_KEYS.map((key) => ({
        value: key,
        label: t(`vault.security_log.range.${key}`),
      })),
    [t],
  );

  if (isLocked || error?.code === "locked") {
    return (
      <Typography.Paragraph type="secondary">
        {t("vault.security_log.locked_hint")}
      </Typography.Paragraph>
    );
  }

  const visibleRows = rows ?? [];

  return (
    <div>
      <Typography.Paragraph type="secondary">
        {t("vault.security_log.description")}
      </Typography.Paragraph>

      <Space wrap style={{ marginBottom: 12 }}>
        <Select
          mode="multiple"
          allowClear
          showSearch={{ optionFilterProp: "label" }}
          maxTagCount="responsive"
          style={{ minWidth: 220 }}
          placeholder={t("vault.security_log.event_placeholder")}
          value={eventTypes}
          onChange={setEventTypes}
          options={eventOptions}
        />
        <Select<RangeKey>
          style={{ width: 140 }}
          value={range}
          onChange={setRange}
          options={rangeOptions}
        />
        <Select<number>
          style={{ width: 120 }}
          value={limit}
          onChange={setLimit}
          options={LIMIT_OPTIONS.map((value) => ({
            value,
            label: t("vault.security_log.limit_value", { count: value }),
          }))}
        />
        <Button
          icon={<ReloadOutlined />}
          loading={loading}
          onClick={() => setReloadToken((token) => token + 1)}
        >
          {t("vault.security_log.refresh")}
        </Button>
      </Space>

      {error !== null && (
        <Alert
          type="error"
          showIcon
          style={{ marginBottom: 12 }}
          title={t("vault.security_log.load_failed")}
          description={error.text}
        />
      )}

      {loading && rows === null ? (
        <div style={{ display: "flex", justifyContent: "center", padding: 24 }}>
          <Spin />
        </div>
      ) : visibleRows.length === 0 ? (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={t("vault.security_log.empty")}
        />
      ) : (
        <div className={styles.borderedList}>
          {visibleRows.map((row) => {
            const targetId = openableEntryId(row.entryId);
            const clickable = targetId !== null && onOpenEntry !== undefined;
            const activate = () => {
              if (targetId === null || onOpenEntry === undefined) return;
              onOpenEntry(targetId);
            };
            return (
              <div
                key={row.id}
                className={
                  clickable
                    ? `${styles.borderedListItem} ${styles.auditRowClickable}`
                    : styles.borderedListItem
                }
                role={clickable ? "button" : undefined}
                tabIndex={clickable ? 0 : undefined}
                onClick={clickable ? activate : undefined}
                onKeyDown={
                  clickable
                    ? (event) => {
                        if (event.key !== "Enter" && event.key !== " ") return;
                        event.preventDefault();
                        activate();
                      }
                    : undefined
                }
              >
                <div className={styles.listItemMeta}>
                  <div className={styles.listItemMetaTitle}>
                    <span>{vaultAuditEventText(row.eventType)}</span>
                    {isBackfilled(row.id) && (
                      <Tag
                        color="default"
                        title={t("vault.security_log.backfill_hint")}
                        style={{ marginInlineStart: 8 }}
                      >
                        {t("vault.security_log.backfill_badge")}
                      </Tag>
                    )}
                  </div>
                  <div className={styles.listItemMetaDescription}>
                    {`${formatTime(row.timestamp)} · ${detailText(row.detail)}`}
                  </div>
                </div>
                {clickable ? (
                  <Typography.Text type="secondary" title={targetId}>
                    {t("vault.security_log.open_entry")}
                  </Typography.Text>
                ) : (
                  row.entryId !== null &&
                  row.entryId.length > 0 && (
                    <Typography.Text type="secondary" code title={row.entryId}>
                      {row.entryId}
                    </Typography.Text>
                  )
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
