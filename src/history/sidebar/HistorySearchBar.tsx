import React from "react";
import { useTranslation } from "react-i18next";
import { Input, Button, Tooltip } from "antd";
import { SearchOutlined, ReloadOutlined } from "@ant-design/icons";
import css from "./HistorySearchBar.module.css";

interface HistorySearchBarProps {
  searchQuery: string;
  onSearch: (value: string) => void;
  searchFullText: boolean;
  onToggleFullText: () => void;
  refreshing: boolean;
  onRefresh: () => void;
}

export const HistorySearchBar: React.FC<HistorySearchBarProps> = React.memo(
  ({
    searchQuery,
    onSearch,
    searchFullText,
    onToggleFullText,
    refreshing,
    onRefresh,
  }) => {
    const { t } = useTranslation();

    return (
      <div className={css.searchBar}>
        <Input
          prefix={<SearchOutlined />}
          placeholder={t("history.search_placeholder")}
          allowClear
          value={searchQuery}
          onChange={(e) => onSearch(e.target.value)}
          size="small"
          className={css.searchInput}
          suffix={
            <span
              onClick={(e) => {
                e.stopPropagation();
                onToggleFullText();
              }}
              className={css.searchModeToggle}
            >
              {searchFullText
                ? t("history.search_fulltext")
                : t("history.search_title")}
            </span>
          }
        />
        <Tooltip title={t("history.refresh_list")}>
          <Button
            type="text"
            size="small"
            icon={<ReloadOutlined spin={refreshing} />}
            onClick={onRefresh}
            style={{ flexShrink: 0 }}
          />
        </Tooltip>
      </div>
    );
  },
);
