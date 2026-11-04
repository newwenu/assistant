import React from "react";
import { Input } from "antd";
import { SearchOutlined } from "@ant-design/icons";
import { translateKey } from "@/i18n";
import { useToolStore } from "@/stores/toolStore";

interface ToolSearchBarProps {
  placeholder?: string;
  size?: "small" | "middle" | "large";
}

export const ToolSearchBar: React.FC<ToolSearchBarProps> = ({
  placeholder = translateKey("tools.search_placeholder"),
  size = "large",
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
      size={size}
      style={{ marginBottom: 16 }}
    />
  );
};
