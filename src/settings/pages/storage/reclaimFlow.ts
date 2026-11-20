import { translateKey, type I18nPath } from "@/i18n";
import type {
  StorageReclaimActionDto,
  StorageReclaimEntryResultDto,
  StorageReclaimPlanEntryDto,
  StorageReclaimResultDto,
} from "@/types/protocol.contract";
import { formatBytes } from "@/utils/formatBytes";
import { actionText, entryNameText, reclaimErrorCodeText } from "./lexicon";

export type ReclaimPhase = "idle" | "planning" | "confirming" | "executing";

export interface ReclaimState {
  phase: ReclaimPhase;
  key: string | null;
  action: StorageReclaimActionDto | null;
  plan: StorageReclaimPlanEntryDto | null;
  typedKey: string;
}

export type ReclaimEvent =
  | { type: "request"; key: string; action: StorageReclaimActionDto }
  | { type: "planned"; entry: StorageReclaimPlanEntryDto }
  | { type: "typed"; value: string }
  | { type: "execute" }
  | { type: "failed"; keepDialogOpen: boolean }
  | { type: "succeeded" }
  | { type: "cancel" };

export const INITIAL_RECLAIM_STATE: ReclaimState = {
  phase: "idle",
  key: null,
  action: null,
  plan: null,
  typedKey: "",
};

export function reclaimReducer(
  state: ReclaimState,
  event: ReclaimEvent,
): ReclaimState {
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
        : INITIAL_RECLAIM_STATE;
    case "succeeded":
    case "cancel":
      if (state.phase === "executing" && event.type === "cancel") return state;
      return INITIAL_RECLAIM_STATE;
    default:
      return state;
  }
}

export function dialogOpen(state: ReclaimState): boolean {
  if (state.phase === "confirming") return true;
  return state.phase === "executing" && state.plan?.confirmation !== "none";
}

export function canExecute(state: ReclaimState): boolean {
  if (state.phase !== "confirming" || state.plan === null) return false;
  if (state.plan.confirmation !== "strongConfirm") return true;
  return state.typedKey === state.plan.key;
}

export type ReclaimStage = "plan" | "execute";

export interface ReclaimErrorView {
  messageKey: I18nPath;
  detail: string | null;
  keepDialogOpen: boolean;
}

const PLAN_ERROR_KEYS = {
  busy: "settings.storage.reclaim.error.busy",
  planExpired: "settings.storage.reclaim.error.plan_expired",
  planNotFound: "settings.storage.reclaim.error.plan_not_found",
  entryNotAllowed: "settings.storage.reclaim.error.not_allowed",
} as const satisfies Record<string, I18nPath>;

const EXECUTE_ERROR_KEYS = {
  ...PLAN_ERROR_KEYS,
  confirmationRequired: "settings.storage.reclaim.error.confirmation_required",
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

export function reclaimErrorView(
  stage: ReclaimStage,
  raw: unknown,
): ReclaimErrorView {
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
      messageKey: "settings.storage.reclaim.error.plan_failed",
      detail: errorDetailOf(raw),
      keepDialogOpen: false,
    };
  }

  return {
    messageKey: "settings.storage.reclaim.error.execute_failed",
    detail: errorDetailOf(raw),
    keepDialogOpen: false,
  };
}

export type ReclaimNoticeKind = "success" | "warning" | "error" | "info";

export interface ReclaimResultNotice {
  kind: ReclaimNoticeKind;
  text: string;
  detail: string | null;
}

function failureDetail(entry: StorageReclaimEntryResultDto): string | null {
  const first = entry.errors[0];
  return first === undefined ? null : reclaimErrorCodeText(first.code);
}

export function reclaimResultNotice(
  result: StorageReclaimResultDto,
): ReclaimResultNotice {
  const entry = result.entries[0];
  if (entry === undefined) {
    console.warn("[storage] reclaim result carries no entry");
    return {
      kind: "error",
      text: translateKey("settings.storage.reclaim.result.failed"),
      detail: null,
    };
  }

  switch (entry.status) {
    case "done":
      return {
        kind: "success",
        text: translateKey("settings.storage.reclaim.result.done", {
          count: entry.removedCount,
          size: formatBytes(entry.removedBytes),
        }),
        detail: null,
      };
    case "partial":
      return {
        kind: "warning",
        text: translateKey("settings.storage.reclaim.result.partial", {
          count: entry.removedCount,
          size: formatBytes(entry.removedBytes),
          failed: entry.failedCount,
        }),
        detail: failureDetail(entry),
      };
    case "failed":
      return {
        kind: "error",
        text: translateKey("settings.storage.reclaim.result.failed"),
        detail: failureDetail(entry),
      };
    case "skipped":
      return {
        kind: "info",
        text: translateKey("settings.storage.reclaim.result.skipped"),
        detail: null,
      };
    case "notEligible":
      console.warn(
        "[storage] reclaim returned notEligible for a planned entry",
      );
      return {
        kind: "error",
        text: translateKey("settings.storage.reclaim.error.not_allowed"),
        detail: null,
      };
    default: {
      const status: string = entry.status;
      console.warn(`[storage] unknown reclaim status: "${status}"`);
      return {
        kind: "error",
        text: translateKey("settings.storage.reclaim.result.failed"),
        detail: null,
      };
    }
  }
}

export function dialogTitle(state: ReclaimState): string {
  if (state.plan === null) return "";
  const name = entryNameText(state.plan.key);
  return state.plan.confirmation === "strongConfirm"
    ? translateKey("settings.storage.reclaim.strong.title", { entry: name })
    : translateKey("settings.storage.reclaim.dialog.title", { entry: name });
}

export function dialogBody(state: ReclaimState): string {
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
    ? translateKey("settings.storage.reclaim.strong.body", {
        action,
        count,
        size,
        key: state.plan.key,
      })
    : translateKey("settings.storage.reclaim.dialog.body", {
        action,
        count,
        size,
      });
}
