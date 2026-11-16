import { useMemo, useEffect, useState, useRef } from "react";
import { Button, Empty, Spin, App, Alert, Modal, Space } from "antd";
import { PlusOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useVaultShallow } from "./core/vaultStore";
import TrashDrawer from "./TrashDrawer";
import VaultConflictResolve from "./settings/sync/VaultConflictResolve";
import VaultToolbar from "./VaultToolbar";
import VaultAlerts from "./VaultAlerts";
import VaultActiveFilters from "./VaultActiveFilters";
import VaultCategoryBar from "./VaultCategoryBar";
import VaultEntryRow from "./VaultEntryRow";
import styles from "./vault.module.css";

const SEARCH_DEBOUNCE_MS = 250;

interface VaultListProps {
  onAdd: () => void;
  onEdit: (id: string) => void;
  onSelect: (id: string) => void;
  onDeleted?: (id: string) => void;
  onOpenManage: () => void;
  selectedId?: string | null;
}

export default function VaultList({
  onAdd,
  onEdit,
  onSelect,
  onDeleted,
  onOpenManage,
  selectedId,
}: VaultListProps) {
  const { t } = useTranslation();
  const { modal, message } = App.useApp();
  const [trashOpen, setTrashOpen] = useState(false);
  const [conflictPickerOpen, setConflictPickerOpen] = useState(false);
  const [conflictPath, setConflictPath] = useState<string | null>(null);
  const [removalConfirmOpen, setRemovalConfirmOpen] = useState(false);
  const {
    entries,
    loading,
    searchQuery,
    categoryFilter,
    tagFilter,
    onlyLeaked,
    sortBy,
    hibpLeaks,
    hibpAuditing,
    hibpProgress,
    syncReadyness,
    syncConflicts,
    pendingRemovals,
    syncing,
    loadEntries,
    loadTotalEntryCount,
    setSearchQuery,
    setCategoryFilter,
    setTagFilter,
    setOnlyLeaked,
    setSortBy,
    copyPassword,
    deleteEntry,
    resetAutoLockTimer,
    syncNow,
    loadSyncReadyness,
    loadPendingRemovals,
    loadSyncLastTime,
    confirmRemovals,
    hibpAudit,
    hibpAuditCancel,
  } = useVaultShallow((s) => ({
    entries: s.entries,
    loading: s.loading,
    searchQuery: s.searchQuery,
    categoryFilter: s.categoryFilter,
    tagFilter: s.tagFilter,
    onlyLeaked: s.onlyLeaked,
    sortBy: s.sortBy,
    hibpLeaks: s.hibpLeaks,
    hibpAuditing: s.hibpAuditing,
    hibpProgress: s.hibpProgress,
    syncReadyness: s.syncReadyness,
    syncConflicts: s.syncConflicts,
    pendingRemovals: s.pendingRemovals,
    syncing: s.syncing,
    loadEntries: s.loadEntries,
    loadTotalEntryCount: s.loadTotalEntryCount,
    setSearchQuery: s.setSearchQuery,
    setCategoryFilter: s.setCategoryFilter,
    setTagFilter: s.setTagFilter,
    setOnlyLeaked: s.setOnlyLeaked,
    setSortBy: s.setSortBy,
    copyPassword: s.copyPassword,
    deleteEntry: s.deleteEntry,
    resetAutoLockTimer: s.resetAutoLockTimer,
    syncNow: s.syncNow,
    loadSyncReadyness: s.loadSyncReadyness,
    loadPendingRemovals: s.loadPendingRemovals,
    loadSyncLastTime: s.loadSyncLastTime,
    confirmRemovals: s.confirmRemovals,
    hibpAudit: s.hibpAudit,
    hibpAuditCancel: s.hibpAuditCancel,
  }));

  const initialLoadDone = useRef(false);

  useEffect(() => {
    void loadSyncReadyness();
    void loadPendingRemovals();
    void loadSyncLastTime();
    void loadTotalEntryCount();
  }, [
    loadSyncReadyness,
    loadPendingRemovals,
    loadSyncLastTime,
    loadTotalEntryCount,
  ]);

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

  const leakCount = Object.values(hibpLeaks).filter(
    (count) => count > 0,
  ).length;

  useEffect(() => {
    if (onlyLeaked && leakCount === 0) setOnlyLeaked(false);
  }, [leakCount, onlyLeaked, setOnlyLeaked]);

  const repoState =
    syncReadyness !== null && typeof syncReadyness === "object"
      ? syncReadyness.repoState
      : null;

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

  const hasActiveFilters =
    searchQuery !== "" ||
    categoryFilter !== null ||
    tagFilter !== null ||
    onlyLeaked;

  const clearFilters = () => {
    setSearchQuery("");
    setCategoryFilter(null);
    setTagFilter(null);
    setOnlyLeaked(false);
  };

  const handleDelete = (id: string) => {
    modal.confirm({
      title: t("vault.entry.confirm_delete"),
      okType: "danger",
      onOk: () =>
        deleteEntry(id)
          .then(() => onDeleted?.(id))
          .catch(() => message.error(t("vault.entry.delete_failed"))),
    });
  };

  const handleConfirmRemovals = async () => {
    if (pendingRemovals === null) return;
    await confirmRemovals(pendingRemovals.entries.map((entry) => entry.id));
    setRemovalConfirmOpen(false);
  };

  const handleSyncClick = () => {
    if (pendingRemovals !== null) {
      setRemovalConfirmOpen(true);
      return;
    }
    void syncNow();
  };

  return (
    <div className={styles.listContainer}>
      <VaultAlerts
        pendingRemovals={pendingRemovals}
        repoState={repoState}
        conflictCount={syncConflicts.length}
        onOpenRemovals={() => setRemovalConfirmOpen(true)}
        onOpenConflicts={() => setConflictPickerOpen(true)}
      />

      <VaultToolbar
        searchQuery={searchQuery}
        syncing={syncing}
        hibpAuditing={hibpAuditing}
        hibpProgress={hibpProgress}
        conflictState={repoState}
        pendingRemovalCount={pendingRemovals?.entries.length ?? 0}
        onSearchChange={setSearchQuery}
        onAdd={onAdd}
        onAudit={() => void hibpAudit()}
        onCancelAudit={() => void hibpAuditCancel()}
        onSync={handleSyncClick}
        onOpenTrash={() => setTrashOpen(true)}
      />

      {hasActiveFilters && (
        <VaultActiveFilters
          searchQuery={searchQuery}
          categoryFilter={categoryFilter}
          tagFilter={tagFilter}
          onlyLeaked={onlyLeaked}
          leakCount={leakCount}
          onSearchClear={() => setSearchQuery("")}
          onCategoryClear={() => setCategoryFilter(null)}
          onTagClear={() => setTagFilter(null)}
          onLeakedClear={() => setOnlyLeaked(false)}
          onClearAll={clearFilters}
        />
      )}

      <VaultCategoryBar
        categoryFilter={categoryFilter}
        leakCount={leakCount}
        onlyLeaked={onlyLeaked}
        sortBy={sortBy}
        onCategoryChange={setCategoryFilter}
        onOnlyLeakedChange={setOnlyLeaked}
        onSortChange={setSortBy}
      />

      {loading ? (
        <Spin className={styles.spin} />
      ) : visibleEntries.length === 0 ? (
        <div className={styles.emptyWrap}>
          {hasActiveFilters ? (
            <Empty description={t("vault.entry.no_matching_entries")}>
              <Button onClick={clearFilters}>
                {t("vault.entry.clear_filters")}
              </Button>
            </Empty>
          ) : (
            <Empty description={t("vault.entry.no_entries")}>
              <Space orientation="vertical" align="center">
                <Button type="primary" icon={<PlusOutlined />} onClick={onAdd}>
                  {t("vault.entry.add_entry")}
                </Button>
                <Button type="link" onClick={onOpenManage}>
                  {t("vault.entry.import_or_clone")}
                </Button>
              </Space>
            </Empty>
          )}
        </div>
      ) : (
        <div className={styles.entryList}>
          {visibleEntries.map((entry) => (
            <VaultEntryRow
              key={entry.id}
              entry={entry}
              selected={selectedId === entry.id}
              leakCount={hibpLeaks[entry.id] ?? 0}
              tagFilter={tagFilter}
              onSelect={(id) => {
                resetAutoLockTimer();
                onSelect(id);
              }}
              onEdit={onEdit}
              onDelete={handleDelete}
              onCopy={(id) => void copyPassword(id)}
              onToggleTag={(tag) =>
                setTagFilter(tagFilter === tag ? null : tag)
              }
            />
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
