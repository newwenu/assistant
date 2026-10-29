import { useEffect, useState } from "react";
import {
  Input,
  Tag,
  List,
  Button,
  Empty,
  Spin,
  App,
  Space,
  Tooltip,
  Alert,
  Modal,
} from "antd";
import {
  SearchOutlined,
  PlusOutlined,
  StarFilled,
  CopyOutlined,
  DeleteOutlined,
  EditOutlined,
  UserOutlined,
  FileTextOutlined,
  CreditCardOutlined,
  IdcardOutlined,
  CodeOutlined,
  ToolOutlined,
  SyncOutlined,
  CloudSyncOutlined,
  WarningOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useVaultStore } from "./core/vaultStore";
import type { VaultCategory } from "./core/vault.types";
import { formatRelativeTime } from "@/utils/formatRelativeTime";
import TrashDrawer from "./TrashDrawer";
import VaultConflictResolve from "./settings/VaultConflictResolve";
import styles from "./vault.module.css";

const CATEGORY_OPTIONS: { value: VaultCategory; label: string }[] = [
  { value: "login", label: "Login" },
  { value: "note", label: "Note" },
  { value: "card", label: "Card" },
  { value: "identity", label: "Identity" },
  { value: "sshKey", label: "SSH Key" },
  { value: "custom", label: "Custom" },
];

const CATEGORY_ICONS: Record<VaultCategory, React.ReactNode> = {
  login: <UserOutlined />,
  note: <FileTextOutlined />,
  card: <CreditCardOutlined />,
  identity: <IdcardOutlined />,
  sshKey: <CodeOutlined />,
  custom: <ToolOutlined />,
};

interface VaultListProps {
  onAdd: () => void;
  onEdit: (id: string) => void;
  onSelect: (id: string) => void;
}

export default function VaultList({ onAdd, onEdit, onSelect }: VaultListProps) {
  const { t } = useTranslation();
  const { modal } = App.useApp();
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
    hibpLeaks,
    syncReadyness,
    syncConflicts,
    pendingRemovals,
    syncLastAt,
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

  useEffect(() => {
    void loadEntries();
    void loadSyncReadyness();
    void loadPendingRemovals();
    void loadSyncLastTime();
  }, [
    searchQuery,
    categoryFilter,
    tagFilter,
    loadEntries,
    loadSyncReadyness,
    loadPendingRemovals,
    loadSyncLastTime,
  ]);

  const handleDelete = (id: string) => {
    modal.confirm({
      title: t("vault.entry.confirm_delete"),
      okType: "danger",
      onOk: () => void deleteEntry(id),
    });
  };

  const handleConfirmRemovals = async () => {
    if (pendingRemovals === null) return;
    await confirmRemovals(pendingRemovals.entries.map((entry) => entry.id));
    setRemovalConfirmOpen(false);
  };

  const leakCount = Object.values(hibpLeaks).filter((c) => c > 0).length;
  const repoState =
    syncReadyness !== null && typeof syncReadyness === "object"
      ? syncReadyness.repoState
      : null;

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
        <Tooltip title={t("vault.entry.trash")}>
          <Button
            icon={<DeleteOutlined />}
            onClick={() => setTrashOpen(true)}
          />
        </Tooltip>
      </div>

      <div className={styles.categoryBar}>
        {CATEGORY_OPTIONS.map((o) => (
          <Tag.CheckableTag
            key={o.value}
            checked={categoryFilter === o.value}
            onChange={(checked) => setCategoryFilter(checked ? o.value : null)}
          >
            {CATEGORY_ICONS[o.value]}
            <span style={{ marginLeft: 4 }}>
              {t(`vault.entry.category_${o.value}`)}
            </span>
          </Tag.CheckableTag>
        ))}
      </div>

      {loading ? (
        <Spin className={styles.spin} />
      ) : entries.length === 0 ? (
        <div className={styles.emptyWrap}>
          <Empty description={t("vault.entry.no_entries")}>
            <Button type="primary" icon={<PlusOutlined />} onClick={onAdd}>
              {t("vault.entry.add_entry")}
            </Button>
          </Empty>
        </div>
      ) : (
        <List
          dataSource={entries}
          className={styles.entryList}
          renderItem={(entry) => (
            <List.Item
              className={styles.entryItem}
              onClick={() => {
                resetAutoLockTimer();
                onSelect(entry.id);
              }}
              actions={[
                <Tooltip key="copy" title={t("common.actions.copy")}>
                  <Button
                    type="text"
                    size="small"
                    icon={<CopyOutlined />}
                    onClick={(e) => {
                      e.stopPropagation();
                      void copyPassword(entry.id);
                    }}
                  />
                </Tooltip>,
                <Tooltip key="edit" title={t("common.actions.edit")}>
                  <Button
                    type="text"
                    size="small"
                    icon={<EditOutlined />}
                    onClick={(e) => {
                      e.stopPropagation();
                      onEdit(entry.id);
                    }}
                  />
                </Tooltip>,
                <Tooltip key="delete" title={t("common.actions.delete")}>
                  <Button
                    type="text"
                    size="small"
                    danger
                    icon={<DeleteOutlined />}
                    onClick={(e) => {
                      e.stopPropagation();
                      handleDelete(entry.id);
                    }}
                  />
                </Tooltip>,
              ]}
            >
              <List.Item.Meta
                avatar={CATEGORY_ICONS[entry.category]}
                title={
                  <span>
                    {entry.isFavorite && (
                      <StarFilled
                        style={{ color: "#faad14", marginRight: 4 }}
                      />
                    )}
                    {hibpLeaks[entry.id] > 0 && (
                      <WarningOutlined
                        style={{ color: "#ff4d4f", marginLeft: 4 }}
                      />
                    )}
                    {entry.title}
                  </span>
                }
                description={
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
                    <span className={styles.entryTime}>
                      {formatRelativeTime(entry.updatedAt)}
                    </span>
                  </span>
                }
              />
            </List.Item>
          )}
        />
      )}

      <div className={styles.syncBar}>
        <div className={styles.syncBarLeft}>
          <span>
            {entries.length} {t("vault.entry.entries_count")}
          </span>
          {leakCount > 0 && (
            <span className={styles.leakWarning}>
              <WarningOutlined style={{ marginRight: 4 }} />
              {leakCount} {t("vault.entry.leaked")}
            </span>
          )}
        </div>
        <Space size={4}>
          {syncLastAt !== null && (
            <Tooltip
              title={t("vault.sync.last_sync", {
                time: new Date(syncLastAt).toLocaleString(),
              })}
            >
              <span className={styles.syncLastAt}>
                {formatRelativeTime(syncLastAt)}
              </span>
            </Tooltip>
          )}
          <Tooltip title={t("vault.hibp.audit_all")}>
            <Button
              type="text"
              size="small"
              icon={<WarningOutlined />}
              onClick={() => void hibpAudit()}
              loading={hibpAuditing}
            />
          </Tooltip>
          {syncReadyness === "ready" && (
            <CloudSyncOutlined style={{ color: "#52c41a" }} />
          )}
          {syncReadyness === "dirty" && (
            <CloudSyncOutlined style={{ color: "#faad14" }} />
          )}
          {syncReadyness === "noSync" && (
            <CloudSyncOutlined style={{ color: "#d9d9d9" }} />
          )}
          <Tooltip title={t("vault.sync.now")}>
            <Button
              type="text"
              size="small"
              icon={<SyncOutlined spin={syncing} />}
              onClick={() => void syncNow()}
              disabled={syncing}
            />
          </Tooltip>
        </Space>
      </div>

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
          <List
            dataSource={syncConflicts}
            renderItem={(file) => (
              <List.Item
                actions={[
                  <Button
                    key="open"
                    type="link"
                    onClick={() => {
                      setConflictPath(file.path);
                      setConflictPickerOpen(false);
                    }}
                  >
                    {t("vault.conflict.resolve")}
                  </Button>,
                ]}
              >
                <span>{file.title ?? file.path}</span>
              </List.Item>
            )}
          />
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
            <List
              dataSource={pendingRemovals.entries}
              renderItem={(entry) => (
                <List.Item>
                  <span>{entry.title ?? entry.id}</span>
                </List.Item>
              )}
            />
          </>
        )}
      </Modal>
    </div>
  );
}
