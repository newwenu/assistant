import { Tag, Select } from "antd";
import { WarningOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import {
  vaultCategoryIcon,
  vaultCategoryOptions,
} from "./core/display/vaultCategory";
import { VAULT_TONES } from "./core/display/vaultColors";
import type { VaultCategory } from "./core/vault.types";
import type { VaultSortBy } from "./core/slices/vaultEntriesSlice";
import styles from "./vault.module.css";

interface VaultCategoryBarProps {
  categoryFilter: VaultCategory | null;
  leakCount: number;
  onlyLeaked: boolean;
  sortBy: VaultSortBy;
  onCategoryChange: (category: VaultCategory | null) => void;
  onOnlyLeakedChange: (value: boolean) => void;
  onSortChange: (sortBy: VaultSortBy) => void;
}

export default function VaultCategoryBar({
  categoryFilter,
  leakCount,
  onlyLeaked,
  sortBy,
  onCategoryChange,
  onOnlyLeakedChange,
  onSortChange,
}: VaultCategoryBarProps) {
  const { t } = useTranslation();

  return (
    <div className={styles.categoryBar}>
      <div className={styles.categoryOptions}>
        <Tag.CheckableTag
          checked={categoryFilter === null}
          onChange={() => onCategoryChange(null)}
        >
          {t("vault.entry.category_all")}
        </Tag.CheckableTag>
        {vaultCategoryOptions(t).map((option) => (
          <Tag.CheckableTag
            key={option.value}
            checked={categoryFilter === option.value}
            onChange={(checked) =>
              onCategoryChange(checked ? option.value : null)
            }
          >
            {vaultCategoryIcon(option.value)}
            <span style={{ marginLeft: 4 }}>{option.label}</span>
          </Tag.CheckableTag>
        ))}
        {leakCount > 0 && (
          <Tag.CheckableTag
            checked={onlyLeaked}
            onChange={onOnlyLeakedChange}
            style={{ color: onlyLeaked ? undefined : VAULT_TONES.leak }}
          >
            <WarningOutlined style={{ marginRight: 4 }} />
            {leakCount} {t("vault.entry.leaked")}
          </Tag.CheckableTag>
        )}
      </div>
      <Select<VaultSortBy>
        value={sortBy}
        onChange={onSortChange}
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
  );
}
