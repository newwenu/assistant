import React from "react";
import { Input } from "antd";
import { SearchOutlined } from "@ant-design/icons";
import { useToolStore } from "@/stores/toolStore";

interface ToolSearchBarProps {
  placeholder?: string;
}

export const ToolSearchBar: React.FC<ToolSearchBarProps> = ({
  placeholder = "搜索工具...",
}) => {
  const searchQuery = useToolStore((s) => s.searchQuery);
  const search = useToolStore((s) => s.search);

  return (
    <Input
      prefix={<SearchOutlined />}
      placeholder={placeholder}
      value={searchQuery}
      onChange={(e) => search(e.target.value)}
      allowClear
      size="large"
      style={{ marginBottom: 16 }}
    />
  );
};
