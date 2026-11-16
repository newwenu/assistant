import React, { Suspense, useEffect } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { Typography, Button, Spin } from "antd";
import { ArrowLeftOutlined } from "@ant-design/icons";
import { translateKey } from "@/i18n";
import { toolsMap } from "../framework/registry";
import { FavoriteButton } from "./FavoriteButton";
import { useToolStore } from "@/stores/toolStore";
import { useWindowModeStore } from "@/stores/system/windowModeStore";
import {
  TOOL_DEFAULT_MAX_WIDTH,
  TOOL_WIDE_MAX_WIDTH,
} from "@/config/constants";
import type { WidthMode } from "../framework/types";
import { ToolCapabilityWrapper } from "./ToolCapabilityWrapper";
import "./tool-page.css";

const WIDTH_MAP: Record<WidthMode, number> = {
  default: TOOL_DEFAULT_MAX_WIDTH,
  wide: TOOL_WIDE_MAX_WIDTH,
  full: 0,
};

export const ToolPage: React.FC = () => {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const recordUsage = useToolStore((s) => s.recordUsage);
  const mode = useWindowModeStore((s) => s.mode);
  const isMini = mode === "mini";
  const tool = id ? toolsMap[id] : undefined;

  useEffect(() => {
    if (id) void recordUsage(id);
  }, [id, recordUsage]);

  if (!tool) {
    return (
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          height: "100%",
        }}
      >
        <Typography.Text type="secondary">
          {translateKey("tools.not_found")}
        </Typography.Text>
      </div>
    );
  }

  const maxWidth = isMini
    ? "100%"
    : tool.widthMode
      ? WIDTH_MAP[tool.widthMode]
      : TOOL_DEFAULT_MAX_WIDTH;

  return (
    <div
      key={id}
      className="tool-page-enter"
      style={{
        padding: isMini ? "8px 12px" : "24px 32px",
        height: "100%",
        overflow: "auto",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          marginBottom: 4,
        }}
      >
        <Button
          type="text"
          icon={<ArrowLeftOutlined />}
          onClick={() => {
            void navigate("/");
          }}
          size="small"
        />
        <Typography.Title
          level={2}
          style={{ margin: 0, fontWeight: 500, fontSize: isMini ? 18 : 28 }}
        >
          {translateKey(tool.name)}
        </Typography.Title>
        {!isMini && (
          <div style={{ marginLeft: "auto" }}>
            <FavoriteButton toolId={tool.id} />
          </div>
        )}
      </div>
      <div
        style={{
          width: isMini ? 120 : 200,
          height: 2,
          background: "var(--color-border-secondary)",
          opacity: 0.2,
          marginBottom: 8,
        }}
      />
      {!isMini && (
        <Typography.Text
          type="secondary"
          style={{ display: "block", marginBottom: 20, fontSize: 14 }}
        >
          {translateKey(tool.description)}
        </Typography.Text>
      )}
      <div
        className="tool-page-container"
        style={{
          maxWidth: maxWidth || "100%",
          margin: maxWidth ? "0 auto" : undefined,
        }}
      >
        <Suspense
          fallback={
            <div
              style={{
                display: "flex",
                justifyContent: "center",
              }}
            >
              <Spin size="large" />
            </div>
          }
        >
          <ToolCapabilityWrapper capabilities={tool.capabilities}>
            <tool.component key={id} />
          </ToolCapabilityWrapper>
        </Suspense>
      </div>
    </div>
  );
};
