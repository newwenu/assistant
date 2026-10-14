import React, { Suspense, useEffect, useState, Component } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { Typography, Button, Spin, Result } from "antd";
import { ArrowLeftOutlined, ReloadOutlined } from "@ant-design/icons";
import { translateKey } from "@/i18n";
import { toolsMap } from "./registry";
import { FavoriteButton } from "./FavoriteButton";
import { useToolStore } from "@/stores/toolStore";
import {
  TOOL_DEFAULT_MAX_WIDTH,
  TOOL_WIDE_MAX_WIDTH,
} from "@/config/constants";
import type { WidthMode } from "./types";

const WIDTH_MAP: Record<WidthMode, number> = {
  default: TOOL_DEFAULT_MAX_WIDTH,
  wide: TOOL_WIDE_MAX_WIDTH,
  full: 0,
};

interface LazyErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
}

class LazyErrorBoundary extends Component<
  { children: React.ReactNode; onRetry: () => void },
  LazyErrorBoundaryState
> {
  state: LazyErrorBoundaryState = { hasError: false, error: null };

  static getDerivedStateFromError(error: Error): LazyErrorBoundaryState {
    return { hasError: true, error };
  }

  render() {
    if (this.state.hasError) {
      return (
        <Result
          status="error"
          title={translateKey("tools.load_failed")}
          subTitle={
            this.state.error?.message || translateKey("tools.retry_hint")
          }
          extra={
            <Button
              type="primary"
              icon={<ReloadOutlined />}
              onClick={this.props.onRetry}
            >
              {translateKey("tools.reload")}
            </Button>
          }
        />
      );
    }
    return this.props.children;
  }
}

export const ToolPage: React.FC = () => {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const recordUsage = useToolStore((s) => s.recordUsage);
  const tool = id ? toolsMap[id] : undefined;
  const [animKey, setAnimKey] = useState(0);

  useEffect(() => {
    if (id) void recordUsage(id);
    setAnimKey((k) => k + 1);
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

  const maxWidth = tool.widthMode
    ? WIDTH_MAP[tool.widthMode]
    : TOOL_DEFAULT_MAX_WIDTH;

  const handleRetry = () => {
    setAnimKey((k) => k + 1);
  };

  return (
    <div
      key={id}
      className="tool-page-enter"
      style={{ padding: "24px 32px", height: "100%", overflow: "auto" }}
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
          style={{ margin: 0, fontWeight: 500, fontSize: 28 }}
        >
          {translateKey(tool.name)}
        </Typography.Title>
        <div style={{ marginLeft: "auto" }}>
          <FavoriteButton toolId={tool.id} />
        </div>
      </div>
      <div
        style={{
          width: 200,
          height: 2,
          background: "var(--color-border-secondary)",
          opacity: 0.2,
          marginBottom: 8,
        }}
      />
      <Typography.Text
        type="secondary"
        style={{ display: "block", marginBottom: 20, fontSize: 14 }}
      >
        {translateKey(tool.description)}
      </Typography.Text>
      <div
        style={{
          maxWidth: maxWidth || "100%",
          margin: maxWidth ? "0 auto" : undefined,
        }}
      >
        <LazyErrorBoundary onRetry={handleRetry}>
          <Suspense
            key={`${id}-${animKey}`}
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
            <tool.component key={id} />
          </Suspense>
        </LazyErrorBoundary>
      </div>
    </div>
  );
};
