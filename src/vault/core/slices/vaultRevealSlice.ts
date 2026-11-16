import { createLogger } from "@/services/system/logger";
import { vaultService } from "../vault.service";
import { vaultErrorCode, vaultErrorMessage } from "../display/vaultError";
import type {
  RevealResult,
  RevealedField,
  VaultErrorCode,
} from "../vault.types";

const log = createLogger("vaultStore");

export interface VaultRevealSlice {
  revealedField: RevealedField | null;
  revealTimerId: ReturnType<typeof setTimeout> | null;

  copyPassword: (id: string) => Promise<void>;
  copyField: (id: string, fieldName: string) => Promise<void>;
  revealPassword: (id: string) => Promise<VaultErrorCode | null>;
  revealNotes: (id: string) => Promise<VaultErrorCode | null>;
  hideRevealedField: () => void;
  clearRevealedField: () => void;
}

type FullState = VaultRevealSlice & {
  error: string | null;
  errorCode: VaultErrorCode | null;
};

export function createRevealSlice(
  set: (
    partial: Partial<FullState> | ((state: FullState) => Partial<FullState>),
  ) => void,
  get: () => FullState,
): VaultRevealSlice {
  const stopTimer = () => {
    const timer = get().revealTimerId;
    if (timer) clearTimeout(timer);
    set({ revealTimerId: null });
  };

  const applyRevealed = (
    entryId: string,
    kind: RevealedField["kind"],
    result: RevealResult,
  ) => {
    stopTimer();
    set({
      revealedField: {
        entryId,
        value: result.value,
        expiresAt: result.expiresAt,
        kind,
        visible: true,
      },
    });

    const delay = Math.max(result.expiresAt - Date.now(), 0);
    const timer = setTimeout(() => {
      const current = get().revealedField;
      if (current && current.expiresAt <= Date.now()) {
        set({ revealedField: null, revealTimerId: null });
      } else {
        set({ revealTimerId: null });
      }
    }, delay);
    set({ revealTimerId: timer });
  };

  const reveal = async (
    entryId: string,
    kind: RevealedField["kind"],
  ): Promise<VaultErrorCode | null> => {
    const cached = get().revealedField;
    if (
      cached &&
      cached.kind === kind &&
      cached.entryId === entryId &&
      cached.expiresAt > Date.now()
    ) {
      set({ revealedField: { ...cached, visible: true } });
      return null;
    }

    set({ error: null, errorCode: null });
    try {
      const result: RevealResult =
        kind === "password"
          ? await vaultService.revealPassword(entryId)
          : await vaultService.revealNotes(entryId);
      applyRevealed(entryId, kind, result);
      return null;
    } catch (e) {
      const code = vaultErrorCode(e);
      log.error(`reveal${kind} failed`, e);
      set({
        error: vaultErrorMessage(e),
        errorCode: code,
        revealedField: null,
      });
      stopTimer();
      return code ?? "internal";
    }
  };

  return {
    revealedField: null,
    revealTimerId: null,

    copyPassword: async (id) => {
      try {
        await vaultService.copyPassword(id);
      } catch (e) {
        log.error("copyPassword failed", e);
        set({ error: vaultErrorMessage(e), errorCode: vaultErrorCode(e) });
      }
    },

    copyField: async (id, fieldName) => {
      try {
        await vaultService.copyField(id, fieldName);
      } catch (e) {
        log.error("copyField failed", e);
        set({ error: vaultErrorMessage(e), errorCode: vaultErrorCode(e) });
      }
    },

    revealPassword: (id) => reveal(id, "password"),

    revealNotes: (id) => reveal(id, "notes"),

    hideRevealedField: () => {
      const current = get().revealedField;
      if (!current) return;
      if (current.expiresAt <= Date.now()) {
        set({ revealedField: null });
        return;
      }
      if (!current.visible) return;
      set({ revealedField: { ...current, visible: false } });
    },

    clearRevealedField: () => {
      stopTimer();
      set({ revealedField: null });
    },
  };
}
