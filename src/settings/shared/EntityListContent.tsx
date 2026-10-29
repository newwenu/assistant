import { Empty, Input, Space, Button } from "antd";
import { PlusOutlined, SearchOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import type React from "react";

export interface SectionConfig<T> {
  label: string;
  items: T[];
  renderRow: (item: T) => React.ReactNode;
}

interface EntityListContentProps<T> {
  items: T[];
  sections: SectionConfig<T>[];
  emptyDescription: string;
}

export function EntityListContent<T>({
  items,
  sections,
  emptyDescription,
}: EntityListContentProps<T>) {
  const nonEmptySections = sections.filter((s) => s.items.length > 0);

  if (items.length === 0) {
    return (
      <Empty
        image={Empty.PRESENTED_IMAGE_SIMPLE}
        description={emptyDescription}
      />
    );
  }

  return (
    <>
      {nonEmptySections.map((section, idx) => (
        <div
          key={section.label}
          style={{
            marginBottom: idx < nonEmptySections.length - 1 ? 16 : 0,
          }}
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
      ))}
    </>
  );
}

interface ListHeaderActionsProps {
  searchQuery: string;
  onSearchChange: (query: string) => void;
  addLabel: string;
  onAdd: () => void;
  searchPlaceholder?: string;
}

export function ListHeaderActions({
  searchQuery,
  onSearchChange,
  addLabel,
  onAdd,
  searchPlaceholder,
}: ListHeaderActionsProps) {
  const { t } = useTranslation();

  return (
    <Space>
      <Input
        placeholder={
          searchPlaceholder ??
          t("settings.entity_list.search_placeholder", { title: "" })
        }
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
  );
}
