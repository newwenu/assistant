import { translateKey, type I18nPath } from "@/i18n";
import type {
  StorageReclaimActionDto,
  StorageReclaimEntryResultDto,
  StorageReclaimPlanEntryDto,
  StorageReclaimResultDto,
} from "@/types/protocol.contract";
import { formatBytes } from "@/utils/formatBytes";
import { createLogger } from "@/services/system/logger";
import { actionText, cleanupErrorCodeText, entryNameText } from "./lexicon";

const storageLog = createLogger("storage");

export type CleanupPhase = "idle" | "planning" | "confirming" | "executing";

export interface CleanupState {
  phase: CleanupPhase;
  key: string | null;
  action: StorageReclaimActionDto | null;
  plan: StorageReclaimPlanEntryDto | null;
  typedKey: string;
}

export type CleanupEvent =
  | { type: "request"; key: string; action: StorageReclaimActionDto }
  | { type: "planned"; entry: StorageReclaimPlanEntryDto }
  | { type: "typed"; value: string }
  | { type: "execute" }
  | { type: "failed"; keepDialogOpen: boolean }
  | { type: "succeeded" }
  | { type: "cancel" };

export const INITIAL_CLEANUP_STATE: CleanupState = {
  phase: "idle",
  key: null,
  action: null,
  plan: null,
  typedKey: "",
};

export function cleanupReducer(
  state: CleanupState,
  event: CleanupEvent,
): CleanupState {
  switch (event.type) {
    case "request":
      if (state.phase !== "idle") return state;
      return {
        phase: "planning",
        key: event.key,
        action: event.action,
        plan: null,
        typedKey: "",
      };
    case "planned":
      if (state.phase !== "planning") return state;
      return {
        ...state,
        phase: event.entry.confirmation === "none" ? "executing" : "confirming",
        plan: event.entry,
      };
    case "typed":
      if (state.phase !== "confirming") return state;
      return { ...state, typedKey: event.value };
    case "execute":
      if (!canExecute(state)) return state;
      return { ...state, phase: "executing" };
    case "failed":
      if (state.phase === "idle") return state;
      return event.keepDialogOpen
        ? { ...state, phase: "confirming", typedKey: "" }
        : INITIAL_CLEANUP_STATE;
    case "succeeded":
    case "cancel":
      if (state.phase === "executing" && event.type === "cancel") return state;
      return INITIAL_CLEANUP_STATE;
    default:
      return state;
  }
}

export function dialogOpen(state: CleanupState): boolean {
  if (state.phase === "confirming") return true;
  return state.phase === "executing" && state.plan?.confirmation !== "none";
}

export function canExecute(state: CleanupState): boolean {
  if (state.phase !== "confirming" || state.plan === null) return false;
  if (state.plan.confirmation !== "strongConfirm") return true;
  return state.typedKey === state.plan.key;
}

export type CleanupStage = "plan" | "execute";

export interface CleanupErrorView {
  messageKey: I18nPath;
  detail: string | null;
  keepDialogOpen: boolean;
}

const PLAN_ERROR_KEYS = {
  busy: "settings.storage.cleanup.error.busy",
  planExpired: "settings.storage.cleanup.error.plan_expired",
  planNotFound: "settings.storage.cleanup.error.plan_not_found",
  entryNotAllowed: "settings.storage.cleanup.error.not_allowed",
} as const satisfies Record<string, I18nPath>;

const EXECUTE_ERROR_KEYS = {
  ...PLAN_ERROR_KEYS,
  confirmationRequired: "settings.storage.cleanup.error.confirmation_required",
} as const satisfies Record<string, I18nPath>;

export function errorCodeOf(raw: unknown): string | null {
  if (typeof raw === "string") return raw;
  if (raw instanceof Error) return raw.message;
  return null;
}

export function errorDetailOf(raw: unknown): string | null {
  if (typeof raw === "string") return raw;
  if (raw instanceof Error) return raw.message;
  if (raw === null || raw === undefined) return null;
  try {
    const json: unknown = JSON.stringify(raw);
    return typeof json === "string" ? json : null;
  } catch {
    return null;
  }
}

function lookupErrorKey(
  keys: Readonly<Record<string, I18nPath>>,
  code: string,
): I18nPath | undefined {
  return Object.prototype.hasOwnProperty.call(keys, code)
    ? keys[code]
    : undefined;
}

export function cleanupErrorView(
  stage: CleanupStage,
  raw: unknown,
): CleanupErrorView {
  const code = errorCodeOf(raw);
  const keys = stage === "plan" ? PLAN_ERROR_KEYS : EXECUTE_ERROR_KEYS;
  const key = code === null ? undefined : lookupErrorKey(keys, code);

  if (key !== undefined) {
    return {
      messageKey: key,
      detail: stage === "execute" ? errorDetailOf(raw) : null,
      keepDialogOpen: code === "confirmationRequired",
    };
  }

  if (stage === "plan") {
    return {
      messageKey: "settings.storage.cleanup.error.plan_failed",
      detail: errorDetailOf(raw),
      keepDialogOpen: false,
    };
  }

  return {
    messageKey: "settings.storage.cleanup.error.execute_failed",
    detail: errorDetailOf(raw),
    keepDialogOpen: false,
  };
}

export type CleanupNoticeKind = "success" | "warning" | "error" | "info";

export interface CleanupResultNotice {
  kind: CleanupNoticeKind;
  text: string;
  detail: string | null;
}

function failureDetail(entry: StorageReclaimEntryResultDto): string | null {
  const first = entry.errors[0];
  return first === undefined ? null : cleanupErrorCodeText(first.code);
}

export function cleanupResultNotice(
  result: StorageReclaimResultDto,
): CleanupResultNotice {
  const entry = result.entries[0];
  if (entry === undefined) {
    storageLog.warn("cleanup result carries no entry");
    return {
      kind: "error",
      text: translateKey("settings.storage.cleanup.result.failed"),
      detail: null,
    };
  }

  switch (entry.status) {
    case "done":
      return {
        kind: "success",
        text: translateKey("settings.storage.cleanup.result.done", {
          count: entry.removedCount,
          size: formatBytes(entry.removedBytes),
        }),
        detail: null,
      };
    case "partial":
      return {
        kind: "warning",
        text: translateKey("settings.storage.cleanup.result.partial", {
          count: entry.removedCount,
          size: formatBytes(entry.removedBytes),
          failed: entry.failedCount,
        }),
        detail: failureDetail(entry),
      };
    case "failed":
      return {
        kind: "error",
        text: translateKey("settings.storage.cleanup.result.failed"),
        detail: failureDetail(entry),
      };
    case "skipped":
      return {
        kind: "info",
        text: translateKey("settings.storage.cleanup.result.skipped"),
        detail: null,
      };
    case "scheduled":
      // `E14` WebView2 profile：运行期只登记、下次启动执行（§13.3.5）。
      return {
        kind: "info",
        text: translateKey("settings.storage.cleanup.result.scheduled"),
        detail: null,
      };
    case "notEligible":
      storageLog.warn("cleanup returned notEligible for a planned entry");
      return {
        kind: "error",
        text: translateKey("settings.storage.cleanup.error.not_allowed"),
        detail: null,
      };
    default: {
      const status: string = entry.status;
      storageLog.warn(`unknown cleanup status: "${status}"`);
      return {
        kind: "error",
        text: translateKey("settings.storage.cleanup.result.failed"),
        detail: null,
      };
    }
  }
}

export function dialogTitle(state: CleanupState): string {
  if (state.plan === null) return "";
  const name = entryNameText(state.plan.key);
  return state.plan.confirmation === "strongConfirm"
    ? translateKey("settings.storage.cleanup.strong.title", { entry: name })
    : translateKey("settings.storage.cleanup.dialog.title", { entry: name });
}

export function dialogBody(state: CleanupState): string {
  if (state.plan === null) return "";
  const action = actionText(state.plan.action);
  const dash = translateKey("settings.storage.common.bytes_unknown");
  const count =
    state.plan.expectedCount === null ? dash : state.plan.expectedCount;
  const size =
    state.plan.expectedBytes === null
      ? dash
      : formatBytes(state.plan.expectedBytes);

  return state.plan.confirmation === "strongConfirm"
    ? translateKey("settings.storage.cleanup.strong.body", {
        action,
        count,
        size,
        key: state.plan.key,
      })
    : translateKey("settings.storage.cleanup.dialog.body", {
        action,
        count,
        size,
      });
}
