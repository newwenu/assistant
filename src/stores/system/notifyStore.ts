import { create } from "zustand";
import { createLogger } from "../../services/system/logger";

const log = createLogger("notifyStore");

export type NotifyLevel = "error" | "warning" | "info";

interface Notification {
  level: NotifyLevel;
  titleKey: string;
  descriptionKey: string;
  /** 后端已本地化的原文；存在时优先于 `descriptionKey` 展示。 */
  descriptionText?: string;
  options?: Record<string, unknown>;
}

interface NotifyState {
  pending: Notification[];
  notify: (
    level: NotifyLevel,
    titleKey: string,
    descriptionKey: string,
    options?: Record<string, unknown>,
    descriptionText?: string,
  ) => void;
  dequeue: () => Notification | undefined;
}

export const useNotifyStore = create<NotifyState>((set, get) => ({
  pending: [],

  notify: (
    level: NotifyLevel,
    titleKey: string,
    descriptionKey: string,
    options?: Record<string, unknown>,
    descriptionText?: string,
  ): void => {
    log.warn("Notification queued", { level, titleKey, descriptionKey });
    set((state) => ({
      pending: [
        ...state.pending,
        { level, titleKey, descriptionKey, options, descriptionText },
      ],
    }));
  },

  dequeue: (): Notification | undefined => {
    const { pending } = get();
    if (pending.length === 0) return undefined;
    const [first, ...rest] = pending;
    set({ pending: rest });
    return first;
  },
}));

export function notifyError(
  titleKey: string,
  descriptionKey: string,
  options?: Record<string, unknown>,
  descriptionText?: string,
): void {
  useNotifyStore
    .getState()
    .notify("error", titleKey, descriptionKey, options, descriptionText);
}

export function notifyWarning(
  titleKey: string,
  descriptionKey: string,
  options?: Record<string, unknown>,
): void {
  useNotifyStore
    .getState()
    .notify("warning", titleKey, descriptionKey, options);
}

export function notifyInfo(
  titleKey: string,
  descriptionKey: string,
  options?: Record<string, unknown>,
): void {
  useNotifyStore.getState().notify("info", titleKey, descriptionKey, options);
}
