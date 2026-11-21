import { useCallback, useReducer, useRef } from "react";
import { App } from "antd";
import { translateKey } from "@/i18n";
import { storageService } from "@/services/desktop/storageService";
import type {
  StorageReclaimActionDto,
  StorageReclaimPlanEntryDto,
} from "@/types/protocol.contract";
import {
  INITIAL_CLEANUP_STATE,
  canExecute as canExecuteState,
  cleanupErrorView,
  cleanupReducer,
  cleanupResultNotice,
  dialogOpen as dialogOpenState,
  type CleanupErrorView,
  type CleanupResultNotice,
  type CleanupState,
} from "./cleanupFlow";

export interface CleanupHandle {
  state: CleanupState;
  dialogOpen: boolean;
  canExecute: boolean;
  requestCleanup: (key: string, action: StorageReclaimActionDto) => void;
  confirm: () => void;
  cancel: () => void;
  setTypedKey: (value: string) => void;
}

const PLAN_FAILED_VIEW: CleanupErrorView = {
  messageKey: "settings.storage.cleanup.error.plan_failed",
  detail: null,
  keepDialogOpen: false,
};

function noticeText(notice: CleanupResultNotice): string {
  return notice.detail === null
    ? notice.text
    : `${notice.text} · ${notice.detail}`;
}

export function useCleanup(onFinished: () => void): CleanupHandle {
  const [state, dispatch] = useReducer(cleanupReducer, INITIAL_CLEANUP_STATE);
  const { message } = App.useApp();
  const planIdRef = useRef<string | null>(null);
  const activeRef = useRef(false);

  const showError = useCallback(
    (view: CleanupErrorView): void => {
      const detail = view.detail;
      message.error(
        detail === null
          ? translateKey(view.messageKey)
          : translateKey(view.messageKey, { error: detail }),
      );
    },
    [message],
  );

  const showNotice = useCallback(
    (notice: CleanupResultNotice): void => {
      const text = noticeText(notice);
      switch (notice.kind) {
        case "success":
          message.success(text, 4);
          break;
        case "warning":
          message.warning(text, 4);
          break;
        case "info":
          message.info(text, 4);
          break;
        default:
          message.error(text, 4);
      }
    },
    [message],
  );

  const runExecute = useCallback(
    async (
      planId: string,
      entry: StorageReclaimPlanEntryDto,
    ): Promise<void> => {
      try {
        const result = await storageService.reclaimStorage(
          planId,
          [{ key: entry.key, action: entry.action }],
          true,
        );
        dispatch({ type: "succeeded" });
        showNotice(cleanupResultNotice(result));
      } catch (raw) {
        const view = cleanupErrorView("execute", raw);
        showError(view);
        dispatch({
          type: "failed",
          keepDialogOpen: view.keepDialogOpen && entry.confirmation !== "none",
        });
      } finally {
        planIdRef.current = null;
        activeRef.current = false;
        onFinished();
      }
    },
    [onFinished, showError, showNotice],
  );

  const requestCleanup = useCallback(
    (key: string, action: StorageReclaimActionDto): void => {
      if (activeRef.current) return;
      activeRef.current = true;
      planIdRef.current = null;
      dispatch({ type: "request", key, action });

      void (async (): Promise<void> => {
        let planId: string;
        let entry: StorageReclaimPlanEntryDto | null;
        try {
          const plan = await storageService.getStorageReclaimPlan([key]);
          planId = plan.planId;
          entry = plan.entries.find((item) => item.key === key) ?? null;
          if (entry === null) {
            entry = plan.entries[0] ?? null;
          }
        } catch (raw) {
          activeRef.current = false;
          showError(cleanupErrorView("plan", raw));
          dispatch({ type: "failed", keepDialogOpen: false });
          return;
        }

        if (entry === null) {
          activeRef.current = false;
          showError(PLAN_FAILED_VIEW);
          dispatch({ type: "failed", keepDialogOpen: false });
          return;
        }

        planIdRef.current = planId;
        dispatch({ type: "planned", entry });
        if (entry.confirmation === "none") {
          void runExecute(planId, entry);
        }
      })();
    },
    [runExecute, showError],
  );

  const confirm = useCallback((): void => {
    const plan = state.plan;
    const planId = planIdRef.current;
    if (plan === null || planId === null || !canExecuteState(state)) return;
    dispatch({ type: "execute" });
    void runExecute(planId, plan);
  }, [state, runExecute]);

  const cancel = useCallback((): void => {
    if (state.phase === "executing") return;
    planIdRef.current = null;
    activeRef.current = false;
    dispatch({ type: "cancel" });
  }, [state.phase]);

  const setTypedKey = useCallback((value: string): void => {
    dispatch({ type: "typed", value });
  }, []);

  return {
    state,
    dialogOpen: dialogOpenState(state),
    canExecute: canExecuteState(state),
    requestCleanup,
    confirm,
    cancel,
    setTypedKey,
  };
}
