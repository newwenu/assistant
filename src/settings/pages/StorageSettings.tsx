import React, { useCallback, useEffect, useMemo, useState } from "react";
import { Button, Checkbox, Empty, Modal, Space, Spin, Typography } from "antd";
import {
  RightOutlined,
  DownOutlined,
  DeleteOutlined,
  ReloadOutlined,
  WarningOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { SettingsPageShell } from "../shared/layout/SettingsPageShell";
import {
  storageService,
  type StorageBreakdownDto,
  type AttachmentSummary,
  type ConversationAttachmentSummary,
} from "@/services/storageService";
import { formatBytes } from "@/utils/formatBytes";

function formatTimestamp(ts: number): string {
  if (!ts) return "";
  const d = new Date(ts * 1000);
  return d.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

type GroupKey = `orphan` | `conv-${string}`;

export const StorageSettings: React.FC = () => {
  const { t } = useTranslation();
  const [modal, modalContextHolder] = Modal.useModal();

  const [breakdown, setBreakdown] = useState<StorageBreakdownDto | null>(null);
  const [loading, setLoading] = useState(true);
  const [collapsedGroups, setCollapsedGroups] = useState<Set<GroupKey>>(
    new Set(),
  );
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  const loadBreakdown = useCallback(async () => {
    setLoading(true);
    try {
      const data = await storageService.getStorageBreakdown();
      setBreakdown(data);
      setSelectedIds(new Set());
    } catch {
      setBreakdown(null);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadBreakdown();
  }, [loadBreakdown]);

  const toggleGroup = useCallback((key: GroupKey) => {
    setCollapsedGroups((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }, []);

  const toggleAttachment = useCallback((id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const toggleGroupAll = useCallback((ids: string[]) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      const allSelected = ids.every((id) => next.has(id));
      for (const id of ids) {
        if (allSelected) next.delete(id);
        else next.add(id);
      }
      return next;
    });
  }, []);

  const handleDelete = useCallback(() => {
    if (selectedIds.size === 0) return;
    modal.confirm({
      title: t("settings.storage.confirm_delete.title"),
      content: t("settings.storage.confirm_delete.content"),
      okButtonProps: { danger: true },
      onOk: async () => {
        const ids = Array.from(selectedIds);
        const result = await storageService.deleteAttachments(ids);
        await loadBreakdown();
        return result;
      },
    });
  }, [selectedIds, modal, t, loadBreakdown]);

  const orphanGroupKey: GroupKey = "orphan";

  const conversationGroups = useMemo(() => {
    if (!breakdown) return [];
    return [...breakdown.conversations].sort(
      (a, b) => b.totalBytes - a.totalBytes,
    );
  }, [breakdown]);

  const renderAttachmentRow = useCallback(
    (att: AttachmentSummary) => {
      const checked = selectedIds.has(att.attachmentId);
      return (
        <div key={att.attachmentId} className="storage-attachment-row">
          <Checkbox
            className="storage-attachment-checkbox"
            checked={checked}
            onClick={(e) => e.stopPropagation()}
            onChange={() => toggleAttachment(att.attachmentId)}
          />
          <span className="storage-attachment-name">{att.attachmentId}</span>
          <span className="storage-attachment-meta">
            {formatBytes(att.bytes)} · {att.mediaType}
            {att.createdAt ? ` · ${formatTimestamp(att.createdAt)}` : ""}
          </span>
        </div>
      );
    },
    [selectedIds, toggleAttachment],
  );

  const renderGroupHeader = useCallback(
    (
      key: GroupKey,
      label: string,
      count: number,
      totalBytes: number,
      attachmentIds: string[],
      extra?: string,
    ) => {
      const collapsed = collapsedGroups.has(key);
      const allSelected =
        count > 0 && attachmentIds.every((id) => selectedIds.has(id));
      const someSelected =
        !allSelected && attachmentIds.some((id) => selectedIds.has(id));

      return (
        <div className="storage-group-header" onClick={() => toggleGroup(key)}>
          <Checkbox
            className="storage-group-checkbox"
            checked={allSelected}
            indeterminate={someSelected}
            onClick={(e) => e.stopPropagation()}
            onChange={() => toggleGroupAll(attachmentIds)}
          />
          <span className="storage-group-arrow">
            {collapsed ? <RightOutlined /> : <DownOutlined />}
          </span>
          <span className="storage-group-label">{label}</span>
          <span className="storage-group-meta">
            {count} · {formatBytes(totalBytes)}
            {extra ? ` · ${extra}` : ""}
          </span>
        </div>
      );
    },
    [collapsedGroups, selectedIds, toggleGroup, toggleGroupAll],
  );

  const renderConversationGroup = useCallback(
    (conv: ConversationAttachmentSummary) => {
      const key: GroupKey = `conv-${conv.conversationId}`;
      const collapsed = collapsedGroups.has(key);
      const attachmentIds = conv.attachments.map((a) => a.attachmentId);

      return (
        <div key={key} className="storage-group">
          {renderGroupHeader(
            key,
            conv.title || conv.conversationId,
            conv.attachments.length,
            conv.totalBytes,
            attachmentIds,
            formatTimestamp(conv.updatedAt),
          )}
          {!collapsed && (
            <div className="storage-group-content">
              {conv.attachments.map(renderAttachmentRow)}
            </div>
          )}
        </div>
      );
    },
    [collapsedGroups, renderGroupHeader, renderAttachmentRow],
  );

  return (
    <>
      {modalContextHolder}
      <SettingsPageShell
        wide
        extra={
          <Button
            size="small"
            icon={<ReloadOutlined />}
            onClick={() => void loadBreakdown()}
            loading={loading}
          >
            {t("settings.storage.refresh")}
          </Button>
        }
      >
        {loading ? (
          <div style={{ textAlign: "center", padding: 48 }}>
            <Spin />
          </div>
        ) : !breakdown ? (
          <Empty
            image={Empty.PRESENTED_IMAGE_SIMPLE}
            description={t("settings.storage.load_failed")}
          />
        ) : (
          <>
            <div className="storage-summary-row">
              <Typography.Text type="secondary">
                {t("settings.storage.summary", {
                  count: breakdown.totalCount,
                  size: formatBytes(breakdown.totalBytes),
                })}
              </Typography.Text>
              <Space>
                <Button
                  size="small"
                  danger
                  icon={<DeleteOutlined />}
                  disabled={selectedIds.size === 0}
                  onClick={handleDelete}
                >
                  {t("settings.storage.delete_selected")}
                </Button>
              </Space>
            </div>

            {breakdown.totalBytes > 500 * 1024 * 1024 && (
              <div className="storage-summary-row" style={{ marginBottom: 0 }}>
                <Typography.Text type="warning">
                  <WarningOutlined style={{ marginRight: 6 }} />
                  {t("settings.storage.size_warning")}
                </Typography.Text>
              </div>
            )}

            {breakdown.totalCount === 0 ? (
              <Empty
                image={Empty.PRESENTED_IMAGE_SIMPLE}
                description={t("settings.storage.empty")}
              />
            ) : (
              <>
                {breakdown.orphans.length > 0 && (
                  <div className="storage-group">
                    {renderGroupHeader(
                      orphanGroupKey,
                      t("settings.storage.orphan_group"),
                      breakdown.orphans.length,
                      breakdown.orphans.reduce((sum, a) => sum + a.bytes, 0),
                      breakdown.orphans.map((a) => a.attachmentId),
                    )}
                    {!collapsedGroups.has(orphanGroupKey) && (
                      <div className="storage-group-content">
                        {breakdown.orphans.map(renderAttachmentRow)}
                      </div>
                    )}
                  </div>
                )}

                {conversationGroups.map(renderConversationGroup)}
              </>
            )}
          </>
        )}
      </SettingsPageShell>
    </>
  );
};
