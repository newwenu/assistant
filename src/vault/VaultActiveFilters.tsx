import { Tag, Button, Typography } from "antd";
import { CloseOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { vaultCategoryOptions } from "./core/display/vaultCategory";
import type { VaultCategory } from "./core/vault.types";
import styles from "./vault.module.css";

interface VaultActiveFiltersProps {
  searchQuery: string;
  categoryFilter: VaultCategory | null;
  tagFilter: string | null;
  onlyLeaked: boolean;
  leakCount: number;
  onSearchClear: () => void;
  onCategoryClear: () => void;
  onTagClear: () => void;
  onLeakedClear: () => void;
  onClearAll: () => void;
}

export default function VaultActiveFilters({
  searchQuery,
  categoryFilter,
  tagFilter,
  onlyLeaked,
  leakCount,
  onSearchClear,
  onCategoryClear,
  onTagClear,
  onLeakedClear,
  onClearAll,
}: VaultActiveFiltersProps) {
  const { t } = useTranslation();
  const categoryLabel =
    categoryFilter !== null
      ? vaultCategoryOptions(t).find(
          (option) => option.value === categoryFilter,
        )?.label
      : undefined;

  return (
    <div className={styles.activeFilters}>
      <Typography.Text type="secondary">
        {t("vault.entry.current_filters")}
      </Typography.Text>
      {searchQuery !== "" && (
        <Tag closable closeIcon={<CloseOutlined />} onClose={onSearchClear}>
          {t("vault.entry.filter_search", { query: searchQuery })}
        </Tag>
      )}
      {categoryFilter !== null && (
        <Tag closable closeIcon={<CloseOutlined />} onClose={onCategoryClear}>
          {t("vault.entry.filter_category", { category: categoryLabel })}
        </Tag>
      )}
      {tagFilter !== null && (
        <Tag closable closeIcon={<CloseOutlined />} onClose={onTagClear}>
          {t("vault.entry.filter_tag", { tag: tagFilter })}
        </Tag>
      )}
      {onlyLeaked && (
        <Tag
          color="error"
          closable
          closeIcon={<CloseOutlined />}
          onClose={onLeakedClear}
        >
          {t("vault.entry.filter_leaked", { count: leakCount })}
        </Tag>
      )}
      <Button type="link" size="small" onClick={onClearAll}>
        {t("vault.entry.clear_filters")}
      </Button>
    </div>
  );
}
