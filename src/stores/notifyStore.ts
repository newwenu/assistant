import { create } from "zustand";
import { createLogger } from "../services/logger";

const log = createLogger("notifyStore");

export type NotifyLevel = "error" | "warning" | "info";

interface Notification {
  level: NotifyLevel;
  titleKey: string;
  descriptionKey: string;
  options?: Record<string, unknown>;
}

interface NotifyState {
  pending: Notification[];
  notify: (
    level: NotifyLevel,
    titleKey: string,
    descriptionKey: string,
    options?: Record<string, unknown>,
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
  ): void => {
    log.warn("Notification queued", { level, titleKey, descriptionKey });
    set((state) => ({
      pending: [...state.pending, { level, titleKey, descriptionKey, options }],
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
): void {
  useNotifyStore.getState().notify("error", titleKey, descriptionKey, options);
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
