import {
  Component,
  createElement,
  forwardRef,
  lazy,
  type ComponentPropsWithRef,
  type ComponentType,
  type ErrorInfo,
  type ForwardRefExoticComponent,
  type LazyExoticComponent,
} from "react";
import { Button, Typography } from "antd";
import { createLogger } from "@/services/system/logger";
import { translateKey } from "@/i18n";
import {
  createModuleLoader,
  markModuleFailed,
  subscribeToModuleRecovery,
} from "@/utils/moduleLoader";

const log = createLogger("LazyModule");

class ModuleImportError extends Error {
  constructor(readonly originalError: unknown) {
    super(
      originalError instanceof Error
        ? originalError.message
        : String(originalError),
    );
    this.name = "ModuleImportError";
  }
}

function ModuleLoadFailure({
  error,
  retry,
}: {
  error: ModuleImportError;
  retry: () => void;
}) {
  return (
    <div style={{ padding: 16, minWidth: 0, maxWidth: "100%" }}>
      <Typography.Text type="danger">
        {translateKey("common.errors.moduleLoadFailed")}
      </Typography.Text>
      <div style={{ marginTop: 8, display: "flex", gap: 8 }}>
        <Button size="small" type="primary" onClick={retry}>
          {translateKey("common.errors.retry")}
        </Button>
        <Button size="small" onClick={() => window.location.reload()}>
          {translateKey("common.errors.reload")}
        </Button>
      </div>
      {import.meta.env.DEV && (
        <details style={{ marginTop: 12, overflowWrap: "anywhere" }}>
          <summary>{translateKey("common.errors.technicalDetails")}</summary>
          {error.message}
        </details>
      )}
    </div>
  );
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any -- React HOC generic constraint, mirrors React.lazy signature
export type RecoverableLazyComponent<T extends ComponentType<any>> =
  ForwardRefExoticComponent<ComponentPropsWithRef<T>> & {
    preload: () => Promise<{ default: T }>;
  };

// eslint-disable-next-line @typescript-eslint/no-explicit-any -- React HOC generic constraint, mirrors React.lazy signature
export function lazyWithRecovery<T extends ComponentType<any>>(
  loader: () => Promise<{ default: T }>,
): RecoverableLazyComponent<T> {
  const moduleLoader = createModuleLoader(loader);
  const createLazyComponent = (): LazyExoticComponent<T> =>
    lazy(() =>
      moduleLoader.load().catch((error: unknown) => {
        throw new ModuleImportError(error);
      }),
    );
  let Content = createLazyComponent();

  type Props = { componentProps: ComponentPropsWithRef<T> };
  type State = {
    error: ModuleImportError | null;
    content: LazyExoticComponent<T>;
  };

  class ModuleBoundary extends Component<Props, State> {
    state: State = { error: null, content: Content };
    private unsubscribe?: () => void;

    static getDerivedStateFromError(error: unknown) {
      if (!(error instanceof ModuleImportError)) throw error;
      return { error };
    }

    componentDidMount() {
      if (this.state.error) this.listenForRecovery();
    }

    private listenForRecovery() {
      this.unsubscribe?.();
      this.unsubscribe = subscribeToModuleRecovery(this.retry);
    }

    componentDidCatch(error: ModuleImportError, info: ErrorInfo) {
      if (Content === this.state.content) Content = createLazyComponent();
      this.listenForRecovery();
      markModuleFailed(this.retry);
      log.warn("Failed to load view module", {
        error: error.originalError,
        componentStack: info.componentStack,
      });
    }

    componentWillUnmount() {
      this.unsubscribe?.();
    }

    retry = () => {
      this.unsubscribe?.();
      this.unsubscribe = undefined;
      this.setState({ error: null, content: Content });
    };

    render() {
      const { error, content } = this.state;
      if (!error) return createElement(content, this.props.componentProps);

      return <ModuleLoadFailure error={error} retry={this.retry} />;
    }
  }

  const Recoverable = forwardRef((props, ref) => (
    <ModuleBoundary
      componentProps={
        {
          ...props,
          ...(ref === null ? {} : { ref }),
        } as ComponentPropsWithRef<T>
      }
    />
  ));
  Recoverable.displayName = "RecoverableLazy";
  return Object.assign(Recoverable, {
    preload: moduleLoader.load,
  }) as RecoverableLazyComponent<T>;
}
