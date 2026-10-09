import React from "react";
import { Typography, Row, Col, Empty } from "antd";
import { useNavigate } from "react-router-dom";
import {
  DndContext,
  closestCenter,
  PointerSensor,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import {
  SortableContext,
  arrayMove,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { ToolSearchBar } from "./ToolSearchBar";
import { ToolCard } from "./ToolCard";
import { useToolStore } from "@/stores/toolStore";
import { toolsMap } from "./registry";
import type { ToolDefinition } from "./types";
import { TOOL_CARD_GAP } from "@/config/constants";

interface SortableToolCardProps {
  tool: ToolDefinition;
}

const SortableToolCard: React.FC<SortableToolCardProps> = ({ tool }) => {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: tool.id });

  const style: React.CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : 1,
  };

  return (
    <div ref={setNodeRef} style={style} {...attributes} {...listeners}>
      <ToolCard tool={tool} />
    </div>
  );
};

export const ToolBox: React.FC = () => {
  const navigate = useNavigate();
  const allTools = useToolStore((s) => s.tools);
  const toolsByCategory = useToolStore((s) => s.toolsByCategory);
  const favoriteToolIds = useToolStore((s) => s.favoriteToolIds);
  const recentTools = useToolStore((s) => s.recentTools);
  const reorderFavorites = useToolStore((s) => s.reorderFavorites);
  const searchQuery = useToolStore((s) => s.searchQuery);
  const searchResults = useToolStore((s) => s.searchResults);

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 8 } }),
  );

  const favoriteTools = favoriteToolIds
    .map((id) => toolsMap[id])
    .filter(Boolean) as ToolDefinition[];

  const recentToolDefs = recentTools
    .map((id) => toolsMap[id])
    .filter(Boolean) as ToolDefinition[];

  const newTools = allTools.filter((t) => t.isNew);

  const handleDragEnd = (event: any) => {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    const oldIdx = favoriteTools.findIndex((t) => t.id === active.id);
    const newIdx = favoriteTools.findIndex((t) => t.id === over.id);
    if (oldIdx === -1 || newIdx === -1) return;
    const reordered = arrayMove(favoriteTools, oldIdx, newIdx);
    reorderFavorites(reordered.map((t) => t.id));
  };

  const isSearching = searchQuery.trim().length > 0;

  return (
    <div style={{ padding: "20px 24px", overflow: "auto", height: "100%" }}>
      <ToolSearchBar />

      {isSearching ? (
        searchResults.length > 0 ? (
          <Row gutter={[TOOL_CARD_GAP, TOOL_CARD_GAP]}>
            {searchResults.map((tool) => (
              <Col key={tool.id} xs={24} sm={12} md={8} lg={6}>
                <ToolCard tool={tool} />
              </Col>
            ))}
          </Row>
        ) : (
          <Empty description="未找到匹配的工具" />
        )
      ) : (
        <>
          {favoriteTools.length > 0 && (
            <>
              <Typography.Text
                strong
                type="secondary"
                style={{ display: "block", marginBottom: 8, fontSize: 14 }}
              >
                ★ 收藏
              </Typography.Text>
              <DndContext
                sensors={sensors}
                collisionDetection={closestCenter}
                onDragEnd={handleDragEnd}
              >
                <SortableContext
                  items={favoriteTools.map((t) => t.id)}
                  strategy={verticalListSortingStrategy}
                >
                  <Row
                    gutter={[TOOL_CARD_GAP, TOOL_CARD_GAP]}
                    style={{ marginBottom: 20 }}
                  >
                    {favoriteTools.map((tool) => (
                      <Col key={tool.id} xs={24} sm={12} md={8} lg={6}>
                        <SortableToolCard tool={tool} />
                      </Col>
                    ))}
                  </Row>
                </SortableContext>
              </DndContext>
            </>
          )}

          {recentToolDefs.length > 0 && (
            <>
              <Typography.Text
                strong
                type="secondary"
                style={{ display: "block", marginBottom: 8, fontSize: 14 }}
              >
                ⏱ 最近使用
              </Typography.Text>
              <div
                style={{
                  display: "flex",
                  flexWrap: "wrap",
                  gap: 8,
                  marginBottom: 20,
                }}
              >
                {recentToolDefs.map((tool) => (
                  <div
                    key={tool.id}
                    onClick={() => navigate(`/tools/${tool.id}`)}
                    style={{
                      display: "inline-flex",
                      alignItems: "center",
                      gap: 6,
                      padding: "4px 10px",
                      borderRadius: 6,
                      background: "var(--color-fill-quaternary)",
                      cursor: "pointer",
                      fontSize: 13,
                      transition: "background 0.2s",
                    }}
                    onMouseEnter={(e) => {
                      e.currentTarget.style.background =
                        "var(--color-fill-tertiary)";
                    }}
                    onMouseLeave={(e) => {
                      e.currentTarget.style.background =
                        "var(--color-fill-quaternary)";
                    }}
                  >
                    {React.createElement(
                      tool.icon as React.ComponentType<Record<string, unknown>>,
                      {
                        style: {
                          fontSize: 14,
                          color: "var(--color-text-secondary)",
                        },
                      },
                    )}
                    <span>{tool.name}</span>
                  </div>
                ))}
              </div>
            </>
          )}

          {newTools.length > 0 && (
            <>
              <Typography.Text
                strong
                type="secondary"
                style={{ display: "block", marginBottom: 8, fontSize: 14 }}
              >
                新工具
              </Typography.Text>
              <Row
                gutter={[TOOL_CARD_GAP, TOOL_CARD_GAP]}
                style={{ marginBottom: 20 }}
              >
                {newTools.map((tool) => (
                  <Col key={tool.id} xs={24} sm={12} md={8} lg={6}>
                    <ToolCard tool={tool} />
                  </Col>
                ))}
              </Row>
            </>
          )}

          {toolsByCategory.map((group) => (
            <div key={group.category} style={{ marginBottom: 20 }}>
              <Typography.Text
                strong
                type="secondary"
                style={{ display: "block", marginBottom: 8, fontSize: 14 }}
              >
                {group.meta.label}
              </Typography.Text>
              <Row gutter={[TOOL_CARD_GAP, TOOL_CARD_GAP]}>
                {group.tools.map((tool) => (
                  <Col key={tool.id} xs={24} sm={12} md={8} lg={6}>
                    <ToolCard tool={tool} />
                  </Col>
                ))}
              </Row>
            </div>
          ))}
        </>
      )}
    </div>
  );
};
