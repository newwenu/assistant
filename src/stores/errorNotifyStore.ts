import { create } from "zustand";
import { createLogger } from "../services/logger";

const log = createLogger("errorNotifier");

interface ErrorNotification {
  titleKey: string;
  descriptionKey: string;
  options?: Record<string, unknown>;
}

interface ErrorNotifyState {
  pending: ErrorNotification[];
  notifyError: (
    titleKey: string,
    descriptionKey: string,
    options?: Record<string, unknown>,
  ) => void;
  dequeue: () => ErrorNotification | undefined;
}

export const useErrorNotifyStore = create<ErrorNotifyState>((set, get) => ({
  pending: [],

  notifyError: (
    titleKey: string,
    descriptionKey: string,
    options?: Record<string, unknown>,
  ): void => {
    log.warn("Error notification queued", { titleKey, descriptionKey });
    set((state) => ({
      pending: [...state.pending, { titleKey, descriptionKey, options }],
    }));
  },

  dequeue: (): ErrorNotification | undefined => {
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
  useErrorNotifyStore.getState().notifyError(titleKey, descriptionKey, options);
}
