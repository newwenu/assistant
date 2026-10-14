import React from "react";
import { Card, Typography, Tag } from "antd";
import { useNavigate } from "react-router-dom";
import { translateKey } from "@/i18n";
import { FavoriteButton } from "./FavoriteButton";
import type { ToolDefinition } from "./types";

interface ToolCardProps {
  tool: ToolDefinition;
}

export const ToolCard: React.FC<ToolCardProps> = ({ tool }) => {
  const navigate = useNavigate();

  return (
    <Card
      hoverable
      onClick={() => {
        void navigate(`/tools/${tool.id}`);
      }}
      style={{ height: "100%", cursor: "pointer" }}
      styles={{ body: { padding: 16 } }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: 8,
        }}
      >
        {React.createElement(
          tool.icon as React.ComponentType<Record<string, unknown>>,
          {
            style: { fontSize: 28, color: "var(--color-text-secondary)" },
          },
        )}
        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
          {tool.isNew && (
            <Tag color="blue">{translateKey("tools.new_tag")}</Tag>
          )}
          <FavoriteButton toolId={tool.id} />
        </div>
      </div>
      <Typography.Text
        strong
        ellipsis
        style={{ display: "block", marginBottom: 4, fontSize: 15 }}
      >
        {translateKey(tool.name)}
      </Typography.Text>
      <div
        style={{
          display: "-webkit-box",
          WebkitLineClamp: 2,
          WebkitBoxOrient: "vertical",
          overflow: "hidden",
          fontSize: 13,
          color: "var(--color-text-secondary)",
          lineHeight: 1.4,
        }}
      >
        {translateKey(tool.description)}
      </div>
    </Card>
  );
};
