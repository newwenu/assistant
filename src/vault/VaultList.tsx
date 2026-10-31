import { useMemo, useEffect, useState, useRef } from "react";
import {
  Input,
  Tag,
  Button,
  Empty,
  Spin,
  App,
  Tooltip,
  Alert,
  Modal,
  Select,
} from "antd";
import {
  SearchOutlined,
  PlusOutlined,
  StarFilled,
  CopyOutlined,
  DeleteOutlined,
  EditOutlined,
  SafetyOutlined,
  SyncOutlined,
  WarningOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useVaultStore } from "./core/vaultStore";
import { vaultCategoryIcon, vaultCategoryOptions } from "./core/vaultCategory";
import { VAULT_TONES } from "./core/vaultColors";
import { formatRelativeTime } from "@/utils/formatRelativeTime";
import TrashDrawer from "./TrashDrawer";
import VaultConflictResolve from "./settings/VaultConflictResolve";
import styles from "./vault.module.css";

type VaultSortBy = "recent" | "title" | "favorite";

const SEARCH_DEBOUNCE_MS = 250;

interface VaultListProps {
  onAdd: () => void;
  onEdit: (id: string) => void;
  onSelect: (id: string) => void;
  onDeleted?: (id: string) => void;
  selectedId?: string | null;
}

export default function VaultList({
  onAdd,
  onEdit,
  onSelect,
  onDeleted,
  selectedId,
}: VaultListProps) {
  const { t } = useTranslation();
  const { modal } = App.useApp();
  const [trashOpen, setTrashOpen] = useState(false);
  const [conflictPickerOpen, setConflictPickerOpen] = useState(false);
  const [conflictPath, setConflictPath] = useState<string | null>(null);
  const [removalConfirmOpen, setRemovalConfirmOpen] = useState(false);
  const [onlyLeaked, setOnlyLeaked] = useState(false);
  const [sortBy, setSortBy] = useState<VaultSortBy>("recent");
  const {
    entries,
    loading,
    searchQuery,
    categoryFilter,
    tagFilter,
    hibpLeaks,
    syncReadyness,
    syncConflicts,
    pendingRemovals,
    syncing,
    loadEntries,
    setSearchQuery,
    setCategoryFilter,
    setTagFilter,
    copyPassword,
    deleteEntry,
    resetAutoLockTimer,
    syncNow,
    loadSyncReadyness,
    loadPendingRemovals,
    loadSyncLastTime,
    confirmRemovals,
    hibpAudit,
    hibpAuditing,
  } = useVaultStore();

  const initialLoadDone = useRef(false);

  useEffect(() => {
    void loadSyncReadyness();
    void loadPendingRemovals();
    void loadSyncLastTime();
  }, [loadSyncReadyness, loadPendingRemovals, loadSyncLastTime]);

  useEffect(() => {
    const silent = initialLoadDone.current;
    const timer = setTimeout(
      () => {
        initialLoadDone.current = true;
        void loadEntries({ silent });
      },
      silent ? SEARCH_DEBOUNCE_MS : 0,
    );
    return () => clearTimeout(timer);
  }, [searchQuery, categoryFilter, tagFilter, loadEntries]);

  const handleDelete = (id: string) => {
    modal.confirm({
      title: t("vault.entry.confirm_delete"),
      okType: "danger",
      onOk: () => {
        void deleteEntry(id).then(() => onDeleted?.(id));
      },
    });
  };

  const handleConfirmRemovals = async () => {
    if (pendingRemovals === null) return;
    await confirmRemovals(pendingRemovals.entries.map((entry) => entry.id));
    setRemovalConfirmOpen(false);
  };

  const repoState =
    syncReadyness !== null && typeof syncReadyness === "object"
      ? syncReadyness.repoState
      : null;
  const leakCount = Object.values(hibpLeaks).filter((c) => c > 0).length;

  const visibleEntries = useMemo(() => {
    const filtered = onlyLeaked
      ? entries.filter((entry) => (hibpLeaks[entry.id] ?? 0) > 0)
      : entries;
    if (sortBy === "title") {
      return [...filtered].sort((a, b) => a.title.localeCompare(b.title));
    }
    if (sortBy === "favorite") {
      return [...filtered].sort((a, b) => {
        if (a.isFavorite === b.isFavorite) return b.updatedAt - a.updatedAt;
        return a.isFavorite ? -1 : 1;
      });
    }
    return [...filtered].sort((a, b) => b.updatedAt - a.updatedAt);
  }, [entries, onlyLeaked, sortBy, hibpLeaks]);

  return (
    <div className={styles.listContainer}>
      {pendingRemovals !== null && (
        <Alert
          type="warning"
          showIcon
          title={t("vault.sync.pending_removals_banner", {
            count: pendingRemovals.entries.length,
          })}
          description={t("vault.sync.pending_removals_hint")}
          action={
            <Button size="small" onClick={() => setRemovalConfirmOpen(true)}>
              {t("vault.sync.pending_removals_open")}
            </Button>
          }
          style={{ marginBottom: 12 }}
        />
      )}
      {repoState !== null && (
        <Alert
          type="warning"
          showIcon
          title={t("vault.conflict.detected")}
          description={t("vault.conflict.detected_hint", {
            state: repoState,
            count: syncConflicts.length,
          })}
          action={
            <Button size="small" onClick={() => setConflictPickerOpen(true)}>
              {t("vault.conflict.open_resolver")}
            </Button>
          }
          style={{ marginBottom: 12 }}
        />
      )}
      <div className={styles.listHeader}>
        <Input
          prefix={<SearchOutlined />}
          placeholder={t("vault.entry.search")}
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          allowClear
          className={styles.searchInput}
        />
        <Button type="primary" icon={<PlusOutlined />} onClick={onAdd}>
          {t("vault.entry.add_entry")}
        </Button>
        <Tooltip title={t("vault.hibp.audit_all")}>
          <Button
            type="text"
            icon={<SafetyOutlined />}
            onClick={() => void hibpAudit()}
            loading={hibpAuditing}
          />
        </Tooltip>
        <Tooltip
          title={
            repoState !== null
              ? t("vault.conflict.detected")
              : t("vault.sync.now")
          }
        >
          <Button
            type="text"
            danger={repoState !== null}
            icon={<SyncOutlined spin={syncing} />}
            onClick={() => void syncNow()}
            disabled={syncing}
          />
        </Tooltip>
        <Tooltip title={t("vault.entry.trash")}>
          <Button
            icon={<DeleteOutlined />}
            onClick={() => setTrashOpen(true)}
          />
        </Tooltip>
      </div>

      <div className={styles.categoryBar}>
        <div className={styles.categoryOptions}>
          <Tag.CheckableTag
            checked={categoryFilter === null}
            onChange={() => setCategoryFilter(null)}
          >
            {t("vault.entry.category_all")}
          </Tag.CheckableTag>
          {vaultCategoryOptions(t).map((o) => (
            <Tag.CheckableTag
              key={o.value}
              checked={categoryFilter === o.value}
              onChange={(checked) =>
                setCategoryFilter(checked ? o.value : null)
              }
            >
              {vaultCategoryIcon(o.value)}
              <span style={{ marginLeft: 4 }}>{o.label}</span>
            </Tag.CheckableTag>
          ))}
          {leakCount > 0 && (
            <Tag.CheckableTag
              checked={onlyLeaked}
              onChange={setOnlyLeaked}
              style={{ color: onlyLeaked ? undefined : VAULT_TONES.leak }}
            >
              <WarningOutlined style={{ marginRight: 4 }} />
              {leakCount} {t("vault.entry.leaked")}
            </Tag.CheckableTag>
          )}
        </div>
        <Select
          value={sortBy}
          onChange={setSortBy}
          size="small"
          variant="borderless"
          className={styles.sortSelect}
          popupMatchSelectWidth={false}
          options={[
            { value: "recent", label: t("vault.entry.sort_recent") },
            { value: "title", label: t("vault.entry.sort_title") },
            { value: "favorite", label: t("vault.entry.sort_favorite") },
          ]}
        />
      </div>

      {loading ? (
        <Spin className={styles.spin} />
      ) : visibleEntries.length === 0 ? (
        <div className={styles.emptyWrap}>
          <Empty description={t("vault.entry.no_entries")}>
            <Button type="primary" icon={<PlusOutlined />} onClick={onAdd}>
              {t("vault.entry.add_entry")}
            </Button>
          </Empty>
        </div>
      ) : (
        <div className={styles.entryList}>
          {visibleEntries.map((entry) => (
            <div
              key={entry.id}
              className={`${styles.entryItem} ${
                selectedId === entry.id ? styles.entryItemActive : ""
              }`}
              onClick={() => {
                resetAutoLockTimer();
                onSelect(entry.id);
              }}
            >
              <span className={styles.entryAvatar}>
                {vaultCategoryIcon(entry.category)}
              </span>
              <div className={styles.entryMain}>
                <div className={styles.entryTitle}>
                  {entry.isFavorite && (
                    <StarFilled
                      style={{ color: VAULT_TONES.favorite, marginRight: 4 }}
                    />
                  )}
                  {hibpLeaks[entry.id] > 0 && (
                    <WarningOutlined
                      style={{ color: VAULT_TONES.leak, marginLeft: 4 }}
                    />
                  )}
                  {entry.title}
                </div>
                <span className={styles.entryDescription}>
                  {entry.username && (
                    <span className={styles.entryUsername}>
                      {entry.username}
                    </span>
                  )}
                  {entry.url && (
                    <span className={styles.entryUrl}>{entry.url}</span>
                  )}
                  {entry.tags.length > 0 && (
                    <span className={styles.entryTags}>
                      {entry.tags.map((tag) => (
                        <Tag
                          key={tag}
                          color={tagFilter === tag ? "blue" : undefined}
                          style={{ cursor: "pointer" }}
                          onClick={(e) => {
                            e.stopPropagation();
                            setTagFilter(tagFilter === tag ? null : tag);
                          }}
                        >
                          {tag}
                        </Tag>
                      ))}
                    </span>
                  )}
                </span>
              </div>
              <div className={styles.entryActions}>
                <span className={styles.entryTime}>
                  {formatRelativeTime(entry.updatedAt)}
                </span>
                <Tooltip title={t("common.actions.copy")}>
                  <Button
                    type="text"
                    size="small"
                    className={styles.entryActionButton}
                    icon={<CopyOutlined />}
                    onClick={(e) => {
                      e.stopPropagation();
                      void copyPassword(entry.id);
                    }}
                  />
                </Tooltip>
                <Tooltip title={t("common.actions.edit")}>
                  <Button
                    type="text"
                    size="small"
                    className={styles.entryActionButton}
                    icon={<EditOutlined />}
                    onClick={(e) => {
                      e.stopPropagation();
                      onEdit(entry.id);
                    }}
                  />
                </Tooltip>
                <Tooltip title={t("common.actions.delete")}>
                  <Button
                    type="text"
                    size="small"
                    danger
                    className={styles.entryActionButton}
                    icon={<DeleteOutlined />}
                    onClick={(e) => {
                      e.stopPropagation();
                      handleDelete(entry.id);
                    }}
                  />
                </Tooltip>
              </div>
            </div>
          ))}
        </div>
      )}

      <TrashDrawer open={trashOpen} onClose={() => setTrashOpen(false)} />

      <Modal
        open={conflictPickerOpen}
        title={t("vault.conflict.title")}
        footer={null}
        onCancel={() => setConflictPickerOpen(false)}
      >
        {syncConflicts.length === 0 ? (
          <Empty description={t("vault.conflict.no_files")} />
        ) : (
          <div className={styles.simpleList}>
            {syncConflicts.map((file) => (
              <div key={file.path} className={styles.simpleListItem}>
                <span>{file.title ?? file.path}</span>
                <Button
                  type="link"
                  onClick={() => {
                    setConflictPath(file.path);
                    setConflictPickerOpen(false);
                  }}
                >
                  {t("vault.conflict.resolve")}
                </Button>
              </div>
            ))}
          </div>
        )}
      </Modal>

      <VaultConflictResolve
        open={conflictPath !== null}
        conflictPath={conflictPath ?? ""}
        onClose={() => setConflictPath(null)}
      />

      <Modal
        open={removalConfirmOpen && pendingRemovals !== null}
        title={
          pendingRemovals !== null &&
          pendingRemovals.entries.length >= pendingRemovals.totalLocal
            ? t("vault.sync.pending_removals_all_title", {
                count: pendingRemovals.entries.length,
              })
            : t("vault.sync.pending_removals_title", {
                count: pendingRemovals?.entries.length ?? 0,
              })
        }
        okText={t("vault.sync.pending_removals_confirm")}
        cancelText={t("vault.sync.pending_removals_cancel")}
        okButtonProps={{ danger: true }}
        cancelButtonProps={{
          autoFocus:
            pendingRemovals !== null &&
            pendingRemovals.entries.length >= pendingRemovals.totalLocal,
        }}
        confirmLoading={syncing}
        onOk={() => void handleConfirmRemovals()}
        onCancel={() => setRemovalConfirmOpen(false)}
      >
        {pendingRemovals !== null && (
          <>
            {pendingRemovals.entries.length >= pendingRemovals.totalLocal && (
              <Alert
                type="error"
                showIcon
                title={t("vault.sync.pending_removals_all_warning")}
                style={{ marginBottom: 12 }}
              />
            )}
            <p>{t("vault.sync.pending_removals_desc")}</p>
            <div className={styles.simpleList}>
              {pendingRemovals.entries.map((entry) => (
                <div key={entry.id} className={styles.simpleListItem}>
                  <span>{entry.title ?? entry.id}</span>
                </div>
              ))}
            </div>
          </>
        )}
      </Modal>
    </div>
  );
}
