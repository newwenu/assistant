import { createLogger } from "@/services/logger";
import { vaultService } from "../vault.service";
import { vaultErrorCode, vaultErrorMessage } from "../vaultError";
import type {
  RevealResult,
  RevealedField,
  VaultErrorCode,
} from "../vault.types";

const log = createLogger("vaultStore");

export interface VaultRevealSlice {
  revealedField: RevealedField | null;

  copyPassword: (id: string) => Promise<void>;
  copyField: (id: string, fieldName: string) => Promise<void>;
  revealPassword: (id: string) => Promise<void>;
  revealNotes: (id: string) => Promise<void>;
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
  return {
    revealedField: null,

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

    revealPassword: async (id) => {
      try {
        const result: RevealResult = await vaultService.revealPassword(id);
        set({
          revealedField: {
            value: result.value,
            expiresAt: result.expiresAt,
            kind: "password",
          },
        });
        const delay = Math.max(result.expiresAt - Date.now(), 0);
        setTimeout(() => {
          const current = get().revealedField;
          if (current?.kind === "password") get().clearRevealedField();
        }, delay);
      } catch (e) {
        log.error("revealPassword failed", e);
        set({ error: vaultErrorMessage(e), errorCode: vaultErrorCode(e) });
      }
    },

    revealNotes: async (id) => {
      try {
        const result: RevealResult = await vaultService.revealNotes(id);
        set({
          revealedField: {
            value: result.value,
            expiresAt: result.expiresAt,
            kind: "notes",
          },
        });
        const delay = Math.max(result.expiresAt - Date.now(), 0);
        setTimeout(() => {
          const current = get().revealedField;
          if (current?.kind === "notes") get().clearRevealedField();
        }, delay);
      } catch (e) {
        log.error("revealNotes failed", e);
        set({ error: vaultErrorMessage(e), errorCode: vaultErrorCode(e) });
      }
    },

    clearRevealedField: () => set({ revealedField: null }),
  };
}
