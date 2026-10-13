import { Button, Empty, Input, Space, Typography } from "antd";
import { PlusOutlined, SearchOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import type React from "react";

export interface EntityListSectionConfig<T> {
  label: string;
  items: T[];
  renderRow: (item: T) => React.ReactNode;
}

interface EntityListPageShellProps<T> {
  title: string;
  items: T[];
  searchQuery: string;
  onSearchChange: (query: string) => void;
  addLabel: string;
  onAdd: () => void;
  emptyDescription: string;
  sections: EntityListSectionConfig<T>[];
  children?: React.ReactNode;
}

export function EntityListPageShell<T>({
  title,
  items,
  searchQuery,
  onSearchChange,
  addLabel,
  onAdd,
  emptyDescription,
  sections,
  children,
}: EntityListPageShellProps<T>) {
  const { t } = useTranslation();
  const nonEmptySections = sections.filter((s) => s.items.length > 0);

  return (
    <div className="entity-list-page">
      <div className="entity-list-page-header">
        <Typography.Title level={5} style={{ margin: 0 }}>
          {title}
        </Typography.Title>
        <Space>
          <Input
            placeholder={t("settings.entity_list.search_placeholder", {
              title,
            })}
            prefix={<SearchOutlined />}
            value={searchQuery}
            onChange={(e) => onSearchChange(e.target.value)}
            allowClear
            style={{ width: 180 }}
          />
          <Button type="primary" icon={<PlusOutlined />} onClick={onAdd}>
            {addLabel}
          </Button>
        </Space>
      </div>

      {items.length === 0 ? (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={emptyDescription}
        />
      ) : (
        nonEmptySections.map((section, idx) => (
          <div
            key={section.label}
            style={{ marginBottom: idx < nonEmptySections.length - 1 ? 16 : 0 }}
          >
            <div className="entity-list-section-label">
              {section.label}
              <span className="entity-list-section-count">
                {section.items.length}
              </span>
            </div>
            <div className="entity-list-rows">
              {section.items.map(section.renderRow)}
            </div>
          </div>
        ))
      )}

      {children}
    </div>
  );
}
