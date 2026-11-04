import React from "react";
import { Typography, Row, Col, Empty } from "antd";
import { useNavigate } from "react-router-dom";
import { translateKey } from "@/i18n";
import {
  DndContext,
  closestCenter,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
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
import { MiniToolBox } from "./MiniToolBox";
import { useWindowModeStore } from "@/stores/windowModeStore";
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

const NormalToolBox: React.FC = () => {
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
    .filter((t): t is ToolDefinition => Boolean(t));

  const recentToolDefs = recentTools
    .map((id) => toolsMap[id])
    .filter((t): t is ToolDefinition => Boolean(t));

  const newTools = allTools.filter((t) => t.isNew);

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    const oldIdx = favoriteTools.findIndex((t) => t.id === active.id);
    const newIdx = favoriteTools.findIndex((t) => t.id === over.id);
    if (oldIdx === -1 || newIdx === -1) return;
    const reordered = arrayMove(favoriteTools, oldIdx, newIdx);
    void reorderFavorites(reordered.map((t) => t.id));
  };

  const isSearching = searchQuery.trim().length > 0;

  return (
    <div style={{ padding: "20px 24px", overflow: "auto", height: "100%" }}>
      <ToolSearchBar />

      {isSearching ? (
        searchResults.length > 0 ? (
          <Row gutter={[TOOL_CARD_GAP, TOOL_CARD_GAP]}>
            {searchResults.map((tool) => (
              <Col key={`search-${tool.id}`} xs={24} sm={12} md={8} lg={6}>
                <ToolCard tool={tool} />
              </Col>
            ))}
          </Row>
        ) : (
          <Empty description={translateKey("tools.no_match")} />
        )
      ) : (
        <>
          {favoriteTools.length > 0 && (
            <div key="section-favorites" style={{ marginBottom: 20 }}>
              <Typography.Text
                strong
                type="secondary"
                style={{ display: "block", marginBottom: 8, fontSize: 14 }}
              >
                ★ {translateKey("tools.favorites")}
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
                  <Row gutter={[TOOL_CARD_GAP, TOOL_CARD_GAP]}>
                    {favoriteTools.map((tool) => (
                      <Col key={`fav-${tool.id}`} xs={24} sm={12} md={8} lg={6}>
                        <SortableToolCard tool={tool} />
                      </Col>
                    ))}
                  </Row>
                </SortableContext>
              </DndContext>
            </div>
          )}

          {recentToolDefs.length > 0 && (
            <div key="section-recent" style={{ marginBottom: 20 }}>
              <Typography.Text
                strong
                type="secondary"
                style={{ display: "block", marginBottom: 8, fontSize: 14 }}
              >
                ⏱ {translateKey("tools.recent")}
              </Typography.Text>
              <div
                style={{
                  display: "flex",
                  flexWrap: "wrap",
                  gap: 8,
                }}
              >
                {recentToolDefs.map((tool) => (
                  <div
                    key={`recent-${tool.id}`}
                    onClick={() => {
                      void navigate(`/tools/${tool.id}`);
                    }}
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
                    <span>{translateKey(tool.name)}</span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {newTools.length > 0 && (
            <div key="section-new" style={{ marginBottom: 20 }}>
              <Typography.Text
                strong
                type="secondary"
                style={{ display: "block", marginBottom: 8, fontSize: 14 }}
              >
                {translateKey("tools.new_tools")}
              </Typography.Text>
              <Row gutter={[TOOL_CARD_GAP, TOOL_CARD_GAP]}>
                {newTools.map((tool) => (
                  <Col key={`new-${tool.id}`} xs={24} sm={12} md={8} lg={6}>
                    <ToolCard tool={tool} />
                  </Col>
                ))}
              </Row>
            </div>
          )}

          {toolsByCategory.map((group) => (
            <div key={`section-${group.category}`} style={{ marginBottom: 20 }}>
              <Typography.Text
                strong
                type="secondary"
                style={{ display: "block", marginBottom: 8, fontSize: 14 }}
              >
                {translateKey(group.meta.label)}
              </Typography.Text>
              <Row gutter={[TOOL_CARD_GAP, TOOL_CARD_GAP]}>
                {group.tools.map((tool) => (
                  <Col key={`cat-${tool.id}`} xs={24} sm={12} md={8} lg={6}>
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

export const ToolBox: React.FC = () => {
  const mode = useWindowModeStore((s) => s.mode);
  return mode === "mini" ? <MiniToolBox /> : <NormalToolBox />;
};
