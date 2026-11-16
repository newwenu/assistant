import { Button, Tag, Tooltip } from "antd";
import {
  StarFilled,
  CopyOutlined,
  DeleteOutlined,
  EditOutlined,
  WarningOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { vaultCategoryIcon } from "./core/display/vaultCategory";
import { VAULT_TONES } from "./core/display/vaultColors";
import { formatRelativeTime } from "@/utils/formatRelativeTime";
import type { VaultEntrySummary } from "./core/vault.types";
import styles from "./vault.module.css";

interface VaultEntryRowProps {
  entry: VaultEntrySummary;
  selected: boolean;
  leakCount: number;
  tagFilter: string | null;
  onSelect: (id: string) => void;
  onEdit: (id: string) => void;
  onDelete: (id: string) => void;
  onCopy: (id: string) => void;
  onToggleTag: (tag: string) => void;
}

export default function VaultEntryRow({
  entry,
  selected,
  leakCount,
  tagFilter,
  onSelect,
  onEdit,
  onDelete,
  onCopy,
  onToggleTag,
}: VaultEntryRowProps) {
  const { t } = useTranslation();

  const handleSelect = () => onSelect(entry.id);

  return (
    <div
      role="button"
      tabIndex={0}
      className={`${styles.entryItem} ${
        selected ? styles.entryItemActive : ""
      }`}
      onClick={handleSelect}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          handleSelect();
        }
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
          {leakCount > 0 && (
            <WarningOutlined
              style={{ color: VAULT_TONES.leak, marginLeft: 4 }}
            />
          )}
          {entry.title}
        </div>
        <span className={styles.entryDescription}>
          {entry.username && (
            <span className={styles.entryUsername}>{entry.username}</span>
          )}
          {entry.url && <span className={styles.entryUrl}>{entry.url}</span>}
          {entry.tags.length > 0 && (
            <span className={styles.entryTags}>
              {entry.tags.map((tag) => (
                <Tag
                  key={tag}
                  color={tagFilter === tag ? "blue" : undefined}
                  style={{ cursor: "pointer" }}
                  onClick={(event) => {
                    event.stopPropagation();
                    onToggleTag(tag);
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
            aria-label={t("common.actions.copy")}
            onClick={(event) => {
              event.stopPropagation();
              onCopy(entry.id);
            }}
          />
        </Tooltip>
        <Tooltip title={t("common.actions.edit")}>
          <Button
            type="text"
            size="small"
            className={styles.entryActionButton}
            icon={<EditOutlined />}
            aria-label={t("common.actions.edit")}
            onClick={(event) => {
              event.stopPropagation();
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
            aria-label={t("common.actions.delete")}
            onClick={(event) => {
              event.stopPropagation();
              onDelete(entry.id);
            }}
          />
        </Tooltip>
      </div>
    </div>
  );
}
