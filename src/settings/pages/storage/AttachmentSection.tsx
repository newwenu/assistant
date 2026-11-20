import React, { useCallback, useEffect, useMemo, useState } from "react";
import { Alert, App, Button, Checkbox, Empty } from "antd";
import { DeleteOutlined, DownOutlined, RightOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import type {
  AttachmentDeleteResultDto,
  AttachmentSummary,
  ConversationAttachmentSummary,
  StorageBreakdownDto,
} from "@/types/protocol.contract";
import { formatBytes } from "@/utils/formatBytes";
import { formatTimestamp } from "./format";

const SIZE_WARNING_THRESHOLD_BYTES = 500 * 1024 * 1024;

type GroupKey = `orphan` | `conv-${string}`;

const ORPHAN_GROUP_KEY: GroupKey = "orphan";

interface AttachmentSectionProps {
  breakdown: StorageBreakdownDto;
  onDelete: (ids: string[]) => Promise<AttachmentDeleteResultDto>;
}

export const AttachmentSection: React.FC<AttachmentSectionProps> = ({
  breakdown,
  onDelete,
}) => {
  const { t } = useTranslation();
  const { message, modal } = App.useApp();

  const [collapsedGroups, setCollapsedGroups] = useState<Set<GroupKey>>(
    new Set(),
  );
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  useEffect(() => {
    setSelectedIds(new Set());
  }, [breakdown.sampledAt]);

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
    const ids = Array.from(selectedIds);
    modal.confirm({
      title: t("settings.storage.attachments.confirm_delete.title"),
      content: t("settings.storage.attachments.confirm_delete.content"),
      okButtonProps: { danger: true },
      onOk: async () => {
        try {
          const result = await onDelete(ids);
          message.success(
            t("settings.storage.attachments.deleted", {
              count: result.removedCount,
              size: formatBytes(result.removedBytes),
            }),
          );
        } catch {
          message.error(t("settings.storage.attachments.delete_failed"));
        }
      },
    });
  }, [selectedIds, modal, t, onDelete, message]);

  const conversationGroups = useMemo(
    () =>
      [...breakdown.conversations].sort((a, b) => b.totalBytes - a.totalBytes),
    [breakdown.conversations],
  );

  const renderAttachmentRow = (att: AttachmentSummary) => {
    const checked = selectedIds.has(att.attachmentId);
    return (
      <div key={att.attachmentId} className="storage-attachment-row">
        <Checkbox
          className="storage-attachment-checkbox"
          checked={checked}
          onChange={() => toggleAttachment(att.attachmentId)}
        />
        <span className="storage-attachment-name">{att.attachmentId}</span>
        <span className="storage-attachment-meta">
          {formatBytes(att.bytes)} · {att.mediaType}
          {att.createdAt ? ` · ${formatTimestamp(att.createdAt)}` : ""}
        </span>
      </div>
    );
  };

  const renderGroupHeader = (
    key: GroupKey,
    label: string,
    attachmentIds: string[],
    meta: string,
  ) => {
    const collapsed = collapsedGroups.has(key);
    const allSelected =
      attachmentIds.length > 0 &&
      attachmentIds.every((id) => selectedIds.has(id));
    const someSelected =
      !allSelected && attachmentIds.some((id) => selectedIds.has(id));

    return (
      <div className="storage-group-header" onClick={() => toggleGroup(key)}>
        <Checkbox
          className="storage-group-checkbox"
          checked={allSelected}
          indeterminate={someSelected}
          onClick={(event) => event.stopPropagation()}
          onChange={() => toggleGroupAll(attachmentIds)}
        />
        <span className="storage-group-arrow">
          {collapsed ? <RightOutlined /> : <DownOutlined />}
        </span>
        <span className="storage-group-label">{label}</span>
        <span className="storage-group-meta">{meta}</span>
      </div>
    );
  };

  const renderConversationGroup = (conv: ConversationAttachmentSummary) => {
    const key: GroupKey = `conv-${conv.conversationId}`;
    const collapsed = collapsedGroups.has(key);
    const attachmentIds = conv.attachments.map((a) => a.attachmentId);

    return (
      <div key={key} className="storage-group">
        {renderGroupHeader(
          key,
          conv.title || conv.conversationId,
          attachmentIds,
          `${conv.attachments.length} · ${formatBytes(conv.totalBytes)} · ${formatTimestamp(conv.updatedAt)}`,
        )}
        {!collapsed && (
          <div className="storage-group-content">
            {conv.attachments.map(renderAttachmentRow)}
          </div>
        )}
      </div>
    );
  };

  return (
    <section className="storage-section">
      <div className="storage-section-head">
        <div className="storage-section-head-main">
          <span className="storage-section-title">
            {t("settings.storage.attachments.title")}
          </span>
          <span className="storage-section-hint">
            {t("settings.storage.attachments.hint")}
          </span>
        </div>
        <div className="storage-section-head-actions">
          <span className="storage-ledger-total">
            {t("settings.storage.attachments.ledger.total", {
              count: breakdown.totalCount,
              size: formatBytes(breakdown.totalBytes),
            })}
          </span>
          {breakdown.sharedBytes > 0 && (
            <span className="storage-ledger-shared">
              {t("settings.storage.attachments.ledger.shared", {
                size: formatBytes(breakdown.sharedBytes),
              })}
            </span>
          )}
          <Button
            size="small"
            danger
            icon={<DeleteOutlined />}
            disabled={selectedIds.size === 0}
            onClick={handleDelete}
          >
            {t("settings.storage.attachments.delete_selected", {
              count: selectedIds.size,
            })}
          </Button>
        </div>
      </div>

      {breakdown.totalBytes > SIZE_WARNING_THRESHOLD_BYTES && (
        <Alert
          type="warning"
          showIcon
          className="storage-attachment-warning"
          title={t("settings.storage.attachments.size_warning")}
        />
      )}

      {breakdown.totalCount === 0 ? (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={t("settings.storage.attachments.empty")}
        />
      ) : (
        <>
          {breakdown.orphans.length > 0 && (
            <div className="storage-group">
              {renderGroupHeader(
                ORPHAN_GROUP_KEY,
                t("settings.storage.attachments.orphan_group"),
                breakdown.orphans.map((a) => a.attachmentId),
                `${breakdown.orphans.length} · ${formatBytes(
                  breakdown.orphans.reduce((sum, a) => sum + a.bytes, 0),
                )}`,
              )}
              {!collapsedGroups.has(ORPHAN_GROUP_KEY) && (
                <div className="storage-group-content">
                  {breakdown.orphans.map(renderAttachmentRow)}
                </div>
              )}
            </div>
          )}
          {conversationGroups.map(renderConversationGroup)}
        </>
      )}
    </section>
  );
};
