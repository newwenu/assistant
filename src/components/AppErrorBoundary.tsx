import React from "react";
import { createLogger } from "@/services/logger";
import { buildReactCrashLogPayload } from "@/services/reactProductionError";

const log = createLogger("AppErrorBoundary");

interface AppErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
  errorInfo: React.ErrorInfo | null;
}

class AppErrorBoundary extends React.Component<
  { children: React.ReactNode },
  AppErrorBoundaryState
> {
  state: AppErrorBoundaryState = {
    hasError: false,
    error: null,
    errorInfo: null,
  };

  static getDerivedStateFromError(
    error: Error,
  ): Partial<AppErrorBoundaryState> {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo): void {
    this.setState({ errorInfo });
    log.error(
      "[CRASH] React error boundary caught exception",
      buildReactCrashLogPayload(error, errorInfo),
    );
  }

  handleReload = (): void => {
    window.location.reload();
  };

  render(): React.ReactNode {
    if (!this.state.hasError) {
      return this.props.children;
    }

    const { error, errorInfo } = this.state;
    const isDev = import.meta.env.DEV;
    const errorLine = error?.message?.split("\n")[0] ?? "Unknown error";

    return (
      <div
        style={{
          height: "100vh",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          padding: "0 24px",
          fontFamily:
            '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
          color: "#1f1f1f",
          backgroundColor: "#ffffff",
        }}
      >
        <h1
          style={{
            fontSize: 20,
            fontWeight: 600,
            marginBottom: 12,
            textAlign: "center",
          }}
        >
          应用遇到了问题
        </h1>
        <p
          style={{
            fontSize: 14,
            color: "#666",
            marginBottom: 24,
            textAlign: "center",
            maxWidth: 480,
            wordBreak: "break-word",
          }}
        >
          {errorLine}
        </p>
        <button
          onClick={this.handleReload}
          style={{
            fontSize: 14,
            fontWeight: 500,
            padding: "8px 24px",
            border: "none",
            borderRadius: 6,
            backgroundColor: "#1677ff",
            color: "#fff",
            cursor: "pointer",
          }}
        >
          重新加载
        </button>
        {isDev && error && (
          <details
            style={{
              marginTop: 24,
              maxWidth: 600,
              width: "100%",
              fontSize: 12,
              color: "#999",
            }}
          >
            <summary style={{ cursor: "pointer", marginBottom: 8 }}>
              技术详情
            </summary>
            <pre
              style={{
                whiteSpace: "pre-wrap",
                wordBreak: "break-word",
                maxHeight: 300,
                overflow: "auto",
                padding: 12,
                backgroundColor: "#f5f5f5",
                borderRadius: 4,
              }}
            >
              {error.stack}
              {errorInfo?.componentStack &&
                `\n\nComponent stack:${errorInfo.componentStack}`}
            </pre>
          </details>
        )}
      </div>
    );
  }
}

export { AppErrorBoundary };
