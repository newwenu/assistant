import React from "react";
import { createLogger } from "@/services/system/logger";
import { isChatError } from "@/chat/core/chatErrors";
import { translateKey } from "@/i18n";

const log = createLogger("FeatureErrorBoundary");

interface FeatureErrorBoundaryProps {
  children: React.ReactNode;
  featureName: string;
  onError?: (error: Error, errorInfo: React.ErrorInfo) => void;
  fallback?: (error: Error, reset: () => void) => React.ReactNode;
}

interface FeatureErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
}

class FeatureErrorBoundary extends React.Component<
  FeatureErrorBoundaryProps,
  FeatureErrorBoundaryState
> {
  state: FeatureErrorBoundaryState = { hasError: false, error: null };

  static getDerivedStateFromError(
    error: Error,
  ): Partial<FeatureErrorBoundaryState> | null {
    if (isChatError(error)) {
      return null;
    }
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo): void {
    if (isChatError(error)) {
      return;
    }

    log.error(`[FeatureError] ${this.props.featureName} crashed`, {
      name: error.name,
      message: error.message,
      componentStack: errorInfo.componentStack,
    });

    this.props.onError?.(error, errorInfo);
  }

  handleReset = (): void => {
    this.setState({ hasError: false, error: null });
  };

  render(): React.ReactNode {
    if (this.state.hasError && this.state.error) {
      if (this.props.fallback) {
        return this.props.fallback(this.state.error, this.handleReset);
      }

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
          <p style={{ margin: 0, fontSize: 14 }}>
            {translateKey("common.errors.featureCrashed", {
              feature: this.props.featureName,
            })}
          </p>
          <button
            onClick={this.handleReset}
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
            {translateKey("common.errors.retry")}
          </button>
        </div>
      );
    }

    return this.props.children;
  }
}

export { FeatureErrorBoundary };
