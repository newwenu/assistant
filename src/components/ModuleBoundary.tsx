import React from "react";
import { createLogger } from "@/services/logger";
import {
  isModuleLoadError,
  createModuleLoader,
  subscribeToModuleRecovery,
} from "@/utils/moduleLoader";

const log = createLogger("ModuleBoundary");

interface ModuleBoundaryProps {
  children: React.ReactNode;
  loader: () => Promise<unknown>;
}

interface ModuleBoundaryState {
  hasError: boolean;
  error: Error | null;
  retryKey: number;
}

class ModuleBoundary extends React.Component<
  ModuleBoundaryProps,
  ModuleBoundaryState
> {
  private moduleLoader: ReturnType<typeof createModuleLoader>;
  private unsubscribeRecovery: (() => void) | null = null;

  state: ModuleBoundaryState = { hasError: false, error: null, retryKey: 0 };

  constructor(props: ModuleBoundaryProps) {
    super(props);
    this.moduleLoader = createModuleLoader(props.loader);
  }

  static getDerivedStateFromError(
    error: Error,
  ): Partial<ModuleBoundaryState> | null {
    if (isModuleLoadError(error)) {
      return { hasError: true, error };
    }
    return null;
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo): void {
    if (!isModuleLoadError(error)) return;

    log.error("Module import error caught", {
      name: error.name,
      message: error.message,
      componentStack: errorInfo.componentStack,
    });
  }

  componentDidMount(): void {
    this.unsubscribeRecovery = subscribeToModuleRecovery(this.handleRecovery);
  }

  componentWillUnmount(): void {
    this.unsubscribeRecovery?.();
  }

  handleRecovery = (): void => {
    this.moduleLoader.reset();
    this.setState((prev) => ({
      hasError: false,
      error: null,
      retryKey: prev.retryKey + 1,
    }));
  };

  handleRetry = (): void => {
    this.moduleLoader.reset();
    this.setState((prev) => ({
      hasError: false,
      error: null,
      retryKey: prev.retryKey + 1,
    }));
  };

  handleReload = (): void => {
    window.location.reload();
  };

  render(): React.ReactNode {
    if (this.state.hasError && this.state.error) {
      const isDev = import.meta.env.DEV;
      const errorLine =
        this.state.error.message?.split("\n")[0] ?? "Unknown error";

      return (
        <div
          style={{
            padding: 24,
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            height: "100%",
            gap: 12,
            color: "var(--color-text-secondary, #666)",
          }}
        >
          <p style={{ margin: 0, fontSize: 14 }}>页面加载失败</p>
          <div style={{ display: "flex", gap: 8 }}>
            <button
              onClick={this.handleRetry}
              style={{
                fontSize: 13,
                padding: "4px 16px",
                border: "1px solid var(--color-border, #d9d9d9)",
                borderRadius: 4,
                background: "transparent",
                color: "var(--color-text, #1f1f1f)",
                cursor: "pointer",
              }}
            >
              重试
            </button>
            <button
              onClick={this.handleReload}
              style={{
                fontSize: 13,
                padding: "4px 16px",
                border: "1px solid var(--color-border, #d9d9d9)",
                borderRadius: 4,
                background: "transparent",
                color: "var(--color-text, #1f1f1f)",
                cursor: "pointer",
              }}
            >
              重新加载
            </button>
          </div>
          {isDev && (
            <details
              style={{
                marginTop: 8,
                maxWidth: 480,
                width: "100%",
                fontSize: 12,
                color: "#999",
              }}
            >
              <summary style={{ cursor: "pointer", marginBottom: 4 }}>
                技术详情
              </summary>
              <pre
                style={{
                  whiteSpace: "pre-wrap",
                  wordBreak: "break-word",
                  maxHeight: 200,
                  overflow: "auto",
                  padding: 8,
                  backgroundColor: "#f5f5f5",
                  borderRadius: 4,
                  margin: 0,
                }}
              >
                {errorLine}
              </pre>
            </details>
          )}
        </div>
      );
    }

    return (
      <React.Fragment key={this.state.retryKey}>
        {this.props.children}
      </React.Fragment>
    );
  }
}

export { ModuleBoundary };
