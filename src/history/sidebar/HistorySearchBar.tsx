import React, { useImperativeHandle, useRef } from "react";
import { useTranslation } from "react-i18next";
import { Input, Button, Tooltip, Dropdown } from "antd";
import type { InputRef } from "antd";
import {
  SearchOutlined,
  ReloadOutlined,
  CheckSquareOutlined,
  EllipsisOutlined,
} from "@ant-design/icons";
import css from "./HistorySearchBar.module.css";

export interface HistorySearchBarHandle {
  focusSearchInput: () => void;
}

interface HistorySearchBarProps {
  searchQuery: string;
  onSearch: (value: string) => void;
  searchFullText: boolean;
  onToggleFullText: () => void;
  refreshing: boolean;
  onRefresh: () => void;
  multiSelectMode: boolean;
  onToggleMultiSelect: () => void;
}

export const HistorySearchBar = React.forwardRef<
  HistorySearchBarHandle,
  HistorySearchBarProps
>(
  (
    {
      searchQuery,
      onSearch,
      searchFullText,
      onToggleFullText,
      refreshing,
      onRefresh,
      onToggleMultiSelect,
    },
    ref,
  ) => {
    const { t } = useTranslation();
    const inputRef = useRef<InputRef>(null);

    useImperativeHandle(ref, () => ({
      focusSearchInput: () => {
        inputRef.current?.focus();
      },
    }));

    const menuItems = [
      {
        key: "multi-select",
        icon: <CheckSquareOutlined />,
        label: t("history.multi_select"),
        onClick: onToggleMultiSelect,
      },
    ];

    return (
      <div className={css.searchBar}>
        <Tooltip title={t("history.refresh_list")}>
          <Button
            type="text"
            size="small"
            icon={<ReloadOutlined spin={refreshing} />}
            onClick={onRefresh}
            className={css.refreshBtn}
          />
        </Tooltip>
        <Input
          ref={inputRef}
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
        <Dropdown
          menu={{ items: menuItems }}
          trigger={["hover"]}
          placement="bottomRight"
        >
          <Button
            type="text"
            size="small"
            icon={<EllipsisOutlined />}
            className={css.moreBtn}
          />
        </Dropdown>
      </div>
    );
  },
);
