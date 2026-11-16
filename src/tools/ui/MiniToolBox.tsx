import React, { useMemo } from "react";
import { Empty, Typography } from "antd";
import { useNavigate } from "react-router-dom";
import { translateKey } from "@/i18n";
import { ToolSearchBar } from "./ToolSearchBar";
import { toolsMap } from "../framework/registry";
import { useToolStore } from "@/stores/toolStore";
import type { ToolDefinition } from "../framework/types";
import { openTool } from "../framework/miniNavigate";

function toolIcon(tool: ToolDefinition): React.ReactNode {
  return React.createElement(
    tool.icon as React.ComponentType<Record<string, unknown>>,
    {
      style: { fontSize: 18, color: "var(--color-text-secondary)" },
    },
  );
}

export const MiniToolBox: React.FC = () => {
  const navigate = useNavigate();
  const favoriteToolIds = useToolStore((s) => s.favoriteToolIds);
  const recentTools = useToolStore((s) => s.recentTools);
  const searchQuery = useToolStore((s) => s.searchQuery);
  const searchResults = useToolStore((s) => s.searchResults);

  const orderedTools = useMemo(() => {
    const seen = new Set<string>();
    const push = (id: string, out: ToolDefinition[]) => {
      const tool = toolsMap[id];
      if (!tool || seen.has(id)) return;
      seen.add(id);
      out.push(tool);
    };

    const result: ToolDefinition[] = [];
    for (const id of recentTools) push(id, result);
    for (const id of favoriteToolIds) push(id, result);
    for (const tool of Object.values(toolsMap)) push(tool.id, result);
    return result;
  }, [recentTools, favoriteToolIds]);

  const items = searchQuery.trim() ? searchResults : orderedTools;

  return (
    <div style={{ padding: "10px 12px", overflow: "auto", height: "100%" }}>
      <ToolSearchBar size="small" />
      {items.length === 0 ? (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={translateKey("tools.no_match")}
        />
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
          {items.map((tool) => (
            <div
              key={tool.id}
              onClick={() => {
                void openTool(navigate, tool.id);
              }}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 10,
                height: 40,
                padding: "0 8px",
                borderRadius: 6,
                cursor: "pointer",
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.background = "var(--color-bg-hover)";
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.background = "transparent";
              }}
            >
              {toolIcon(tool)}
              <Typography.Text ellipsis style={{ fontSize: 14, margin: 0 }}>
                {translateKey(tool.name)}
              </Typography.Text>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};
