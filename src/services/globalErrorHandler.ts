import { createLogger } from "./logger";
import {
  isMinifiedReactErrorMessage,
  parseMinifiedReactError,
} from "./reactProductionError";

const log = createLogger("global-error");

let whiteScreenLogged = false;

function isRootEmpty(): boolean {
  const root = document.getElementById("root");
  if (!root) return true;
  return root.innerHTML.trim() === "";
}

function scheduleCrashLog(error: Error): void {
  queueMicrotask(() => {
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        if (isRootEmpty() && !whiteScreenLogged) {
          whiteScreenLogged = true;
          log.error("[CRASH] White screen detected after uncaught error", {
            name: error.name,
            message: error.message,
          });
        }
      });
    });
  });
}

function serializeError(reason: unknown): Record<string, unknown> {
  if (reason instanceof Error) {
    return {
      name: reason.name,
      message: reason.message,
      stack: reason.stack,
    };
  }
  return { value: String(reason) };
}

function handleWindowError(event: ErrorEvent | Event): void {
  if (event instanceof ErrorEvent) {
    const msg = event.message ?? "";

    if (isMinifiedReactErrorMessage(msg)) {
      const parsed = parseMinifiedReactError(msg);
      log.error("[CRASH] Minified React error", {
        code: parsed?.code,
        decoderUrl: parsed?.decoderUrl,
        hint: parsed?.hint,
        filename: event.filename,
        lineno: event.lineno,
        colno: event.colno,
      });
    }

    if (event.error instanceof Error) {
      scheduleCrashLog(event.error);
    }
  } else {
    const target = event.target as HTMLElement | null;
    if (!target) return;

    const tagName = target.tagName;
    const src = "src" in target ? (target as HTMLScriptElement).src : undefined;
    const href =
      "href" in target ? (target as HTMLLinkElement).href : undefined;

    if (isRootEmpty()) {
      log.error("[CRASH] Resource load error (white screen)", {
        tagName,
        src: src ?? undefined,
        href: href ?? undefined,
      });
    }
  }
}

function handleUnhandledRejection(event: PromiseRejectionEvent): void {
  const reason: unknown = event.reason;

  if (reason instanceof Error && isMinifiedReactErrorMessage(reason.message)) {
    const parsed = parseMinifiedReactError(reason.message);
    log.error("[CRASH] Minified React error (unhandled rejection)", {
      code: parsed?.code,
      decoderUrl: parsed?.decoderUrl,
      hint: parsed?.hint,
    });
    return;
  }

  log.error("[CRASH] Unhandled promise rejection", serializeError(reason));
}

export function registerGlobalErrorHandlers(): void {
  if (
    (globalThis as Record<string, unknown>)
      .__assistant_global_error_handlers_registered__
  )
    return;

  window.addEventListener("error", handleWindowError, true);
  window.addEventListener("unhandledrejection", handleUnhandledRejection);

  (
    globalThis as Record<string, unknown>
  ).__assistant_global_error_handlers_registered__ = true;
}
