import { createLogger } from "@/services/logger";
import { vaultService } from "../vault.service";
import { vaultErrorCode, vaultErrorMessage } from "../vaultError";
import type {
  TotpCode,
  ParsedOtpAuth,
  StrengthResult,
  VaultErrorCode,
} from "../vault.types";

const log = createLogger("vaultStore");

export interface VaultAuditSlice {
  totpCode: string | null;
  totpTimeRemaining: number;
  totpPeriod: number;
  totpTimerId: ReturnType<typeof setInterval> | null;
  hibpLeaks: Record<string, number>;
  hibpAuditing: boolean;
  hibpProgress: { done: number; total: number } | null;
  hibpCancelRequested: boolean;

  generateTotp: (id: string) => Promise<void>;
  copyTotp: (id: string) => Promise<void>;
  stopTotpTimer: () => void;
  totpParseUri: (uri: string) => Promise<ParsedOtpAuth>;
  queryAuditLog: (
    limit?: number,
    since?: number,
    eventTypes?: string[],
  ) => Promise<import("../vault.types").AuditLogEntry[]>;
  showUnlockWindow: () => Promise<void>;
  hibpCheck: (id: string) => Promise<number>;
  hibpAudit: () => Promise<void>;
  hibpAuditCancel: () => Promise<void>;
  checkMasterPasswordStrength: (
    password: string,
  ) => Promise<StrengthResult | null>;
}

type FullState = VaultAuditSlice & {
  error: string | null;
  errorCode: VaultErrorCode | null;
};

export function createAuditSlice(
  set: (
    partial: Partial<FullState> | ((state: FullState) => Partial<FullState>),
  ) => void,
  get: () => FullState,
): VaultAuditSlice {
  return {
    totpCode: null,
    totpTimeRemaining: 0,
    totpPeriod: 0,
    totpTimerId: null,
    hibpLeaks: {},
    hibpAuditing: false,
    hibpProgress: null,
    hibpCancelRequested: false,

    generateTotp: async (id) => {
      try {
        const result: TotpCode = await vaultService.totpGenerate(id);
        set({
          totpCode: result.code,
          totpTimeRemaining: result.timeRemaining,
          totpPeriod: result.period,
        });

        const { totpTimerId } = get();
        if (totpTimerId) clearInterval(totpTimerId);

        const timerId = setInterval(() => {
          const remaining = get().totpTimeRemaining - 1;
          if (remaining <= 0) {
            void get().generateTotp(id);
          } else {
            set({ totpTimeRemaining: remaining });
          }
        }, 1000);
        set({ totpTimerId: timerId });
      } catch (e) {
        log.error("generateTotp failed", e);
        set({ totpCode: null, totpTimeRemaining: 0, totpPeriod: 0 });
      }
    },

    copyTotp: async (id) => {
      try {
        await vaultService.totpCopy(id);
      } catch (e) {
        log.error("copyTotp failed", e);
        set({ error: vaultErrorMessage(e), errorCode: vaultErrorCode(e) });
      }
    },

    stopTotpTimer: () => {
      const { totpTimerId } = get();
      if (totpTimerId) clearInterval(totpTimerId);
      set({
        totpCode: null,
        totpTimeRemaining: 0,
        totpPeriod: 0,
        totpTimerId: null,
      });
    },

    totpParseUri: async (uri: string) => {
      return vaultService.totpParseUri(uri);
    },

    queryAuditLog: async (
      limit?: number,
      since?: number,
      eventTypes?: string[],
    ) => {
      return vaultService.queryAuditLog(limit, since, eventTypes);
    },

    showUnlockWindow: async () => {
      return vaultService.showUnlockWindow();
    },

    hibpCheck: async (id) => {
      try {
        const count = await vaultService.hibpCheck(id);
        set((state) => ({
          hibpLeaks: { ...state.hibpLeaks, [id]: count },
        }));
        return count;
      } catch (e) {
        log.error("hibpCheck failed", e);
        return 0;
      }
    },

    hibpAudit: async () => {
      set({
        hibpAuditing: true,
        hibpProgress: { done: 0, total: 0 },
        hibpCancelRequested: false,
      });
      try {
        const leaks = await vaultService.hibpAudit();
        if (get().hibpCancelRequested) {
          set({
            hibpAuditing: false,
            hibpProgress: null,
            hibpCancelRequested: false,
          });
          return;
        }
        const leakMap: Record<string, number> = {};
        for (const { entryId, matchCount } of leaks) {
          leakMap[entryId] = matchCount;
        }
        set({ hibpLeaks: leakMap, hibpAuditing: false, hibpProgress: null });
      } catch (e) {
        log.error("hibpAudit failed", e);
        set({
          hibpAuditing: false,
          hibpProgress: null,
          hibpCancelRequested: false,
        });
      }
    },

    hibpAuditCancel: async () => {
      set({ hibpCancelRequested: true });
      try {
        await vaultService.hibpAuditCancel();
      } catch (e) {
        log.error("hibpAuditCancel failed", e);
      }
    },

    checkMasterPasswordStrength: async (password) => {
      try {
        return await vaultService.checkMasterPasswordStrength(password);
      } catch (e) {
        log.error("checkMasterPasswordStrength failed", e);
        return null;
      }
    },
  };
}
