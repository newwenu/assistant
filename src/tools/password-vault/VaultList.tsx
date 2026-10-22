import { useEffect } from "react";
import {
  Input,
  Select,
  Tag,
  List,
  Button,
  Empty,
  Spin,
  Popconfirm,
  Space,
} from "antd";
import {
  SearchOutlined,
  PlusOutlined,
  StarFilled,
  CopyOutlined,
  DeleteOutlined,
  EditOutlined,
  KeyOutlined,
  GlobalOutlined,
  SyncOutlined,
  CloudSyncOutlined,
  WarningOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useVaultStore } from "./vaultStore";
import type { VaultCategory } from "./vault.service";
import styles from "./password-vault.module.css";

const CATEGORY_OPTIONS: { value: VaultCategory; label: string }[] = [
  { value: "login", label: "Login" },
  { value: "note", label: "Note" },
  { value: "card", label: "Card" },
  { value: "identity", label: "Identity" },
  { value: "sshKey", label: "SSH Key" },
  { value: "custom", label: "Custom" },
];

const CATEGORY_ICONS: Record<VaultCategory, React.ReactNode> = {
  login: <KeyOutlined />,
  note: <EditOutlined />,
  card: <GlobalOutlined />,
  identity: <GlobalOutlined />,
  sshKey: <KeyOutlined />,
  custom: <EditOutlined />,
};

interface VaultListProps {
  onAdd: () => void;
  onEdit: (id: string) => void;
  onSelect: (id: string) => void;
}

export default function VaultList({ onAdd, onEdit, onSelect }: VaultListProps) {
  const { t } = useTranslation();
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

  return (
    <div className={styles.listContainer}>
      <div className={styles.listHeader}>
        <Input
          prefix={<SearchOutlined />}
          placeholder={t("tools.password_vault.search")}
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          allowClear
          className={styles.searchInput}
        />
        <Select
          allowClear
          placeholder={t("tools.password_vault.filter_category")}
          value={categoryFilter}
          onChange={(v) => setCategoryFilter(v)}
          options={CATEGORY_OPTIONS.map((o) => ({
            ...o,
            label: t(`tools.password_vault.category_${o.value}`),
          }))}
          className={styles.categorySelect}
        />
        <Button type="primary" icon={<PlusOutlined />} onClick={onAdd}>
          {t("tools.password_vault.add_entry")}
        </Button>
      </div>

      {loading ? (
        <Spin className={styles.spin} />
      ) : entries.length === 0 ? (
        <Empty description={t("tools.password_vault.no_entries")} />
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
                <Button
                  key="copy"
                  type="text"
                  size="small"
                  icon={<CopyOutlined />}
                  onClick={(e) => {
                    e.stopPropagation();
                    void copyPassword(entry.id);
                  }}
                >
                  {t("tools.password_vault.copy")}
                </Button>,
                <Button
                  key="edit"
                  type="text"
                  size="small"
                  icon={<EditOutlined />}
                  onClick={(e) => {
                    e.stopPropagation();
                    onEdit(entry.id);
                  }}
                />,
                <Popconfirm
                  key="delete"
                  title={t("tools.password_vault.confirm_delete")}
                  onConfirm={() => void deleteEntry(entry.id)}
                >
                  <Button
                    type="text"
                    size="small"
                    danger
                    icon={<DeleteOutlined />}
                    onClick={(e) => e.stopPropagation()}
                  />
                </Popconfirm>,
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
                  </span>
                }
              />
            </List.Item>
          )}
        />
      )}

      <div className={styles.syncBar}>
        <Space>
          <span>
            {entries.length} {t("tools.password_vault.entries_count")}
          </span>
          {Object.values(hibpLeaks).filter((c) => c > 0).length > 0 && (
            <span style={{ color: "#ff4d4f" }}>
              <WarningOutlined style={{ marginRight: 4 }} />
              {Object.values(hibpLeaks).filter((c) => c > 0).length}{" "}
              {t("tools.password_vault.leaked")}
            </span>
          )}
          <Button
            type="text"
            size="small"
            icon={<WarningOutlined />}
            onClick={() => void hibpAudit()}
            loading={hibpAuditing}
          >
            {t("tools.password_vault.hibp_audit_all")}
          </Button>
          {syncReadyness === "ready" && (
            <CloudSyncOutlined style={{ color: "#52c41a" }} />
          )}
          {syncReadyness === "dirty" && (
            <CloudSyncOutlined style={{ color: "#faad14" }} />
          )}
          {syncReadyness === "noSync" && (
            <CloudSyncOutlined style={{ color: "#d9d9d9" }} />
          )}
          <Button
            type="text"
            size="small"
            icon={<SyncOutlined spin={syncing} />}
            onClick={() => void syncNow()}
            disabled={syncing}
          >
            {t("tools.password_vault.sync_now")}
          </Button>
        </Space>
      </div>
    </div>
  );
}
