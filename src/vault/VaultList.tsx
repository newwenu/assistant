import { useEffect } from "react";
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
  const {
    entries,
    loading,
    searchQuery,
    categoryFilter,
    tagFilter,
    hibpLeaks,
    syncReadyness,
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
    hibpAudit,
    hibpAuditing,
  } = useVaultStore();

  useEffect(() => {
    void loadEntries();
    void loadSyncReadyness();
  }, [searchQuery, categoryFilter, tagFilter, loadEntries, loadSyncReadyness]);

  const handleDelete = (id: string) => {
    modal.confirm({
      title: t("vault.entry.confirm_delete"),
      okType: "danger",
      onOk: () => void deleteEntry(id),
    });
  };

  const leakCount = Object.values(hibpLeaks).filter((c) => c > 0).length;

  return (
    <div className={styles.listContainer}>
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
    </div>
  );
}
