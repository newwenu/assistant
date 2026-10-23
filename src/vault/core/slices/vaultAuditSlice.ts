import { createLogger } from "@/services/logger";
import { vaultService } from "../vault.service";
import type { TotpResult, ParsedOtpAuth, StrengthResult } from "../vault.types";

const log = createLogger("vaultStore");

export interface VaultAuditSlice {
  totpCode: string | null;
  totpTimeRemaining: number;
  totpTimerId: ReturnType<typeof setInterval> | null;
  hibpLeaks: Record<string, number>;
  hibpAuditing: boolean;

  generateTotp: (
    id: string,
    period?: number,
    digits?: number,
    algorithm?: string,
  ) => Promise<void>;
  copyTotp: (
    id: string,
    period?: number,
    digits?: number,
    algorithm?: string,
  ) => Promise<void>;
  stopTotpTimer: () => void;
  totpParseUri: (uri: string) => Promise<ParsedOtpAuth>;
  queryAuditLog: (
    limit?: number,
  ) => Promise<import("../vault.types").AuditLogEntry[]>;
  showUnlockWindow: () => Promise<void>;
  hibpCheck: (id: string) => Promise<number>;
  hibpAudit: () => Promise<void>;
  checkMasterPasswordStrength: (
    password: string,
  ) => Promise<StrengthResult | null>;
}

type FullState = VaultAuditSlice & {
  error: string | null;
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
    totpTimerId: null,
    hibpLeaks: {},
    hibpAuditing: false,

    generateTotp: async (id, period, digits, algorithm) => {
      try {
        const result: TotpResult = await vaultService.totpGenerate(
          id,
          period,
          digits,
          algorithm,
        );
        set({ totpCode: result.code, totpTimeRemaining: result.timeRemaining });

        const { totpTimerId } = get();
        if (totpTimerId) clearInterval(totpTimerId);

        const timerId = setInterval(() => {
          const remaining = get().totpTimeRemaining - 1;
          if (remaining <= 0) {
            void get().generateTotp(id, period, digits, algorithm);
          } else {
            set({ totpTimeRemaining: remaining });
          }
        }, 1000);
        set({ totpTimerId: timerId });
      } catch (e) {
        log.error("generateTotp failed", e);
        set({ totpCode: null, totpTimeRemaining: 0 });
      }
    },

    copyTotp: async (id, period, digits, algorithm) => {
      try {
        await vaultService.totpCopy(id, period, digits, algorithm);
      } catch (e) {
        log.error("copyTotp failed", e);
        set({ error: String(e) });
      }
    },

    stopTotpTimer: () => {
      const { totpTimerId } = get();
      if (totpTimerId) clearInterval(totpTimerId);
      set({ totpCode: null, totpTimeRemaining: 0, totpTimerId: null });
    },

    totpParseUri: async (uri: string) => {
      return vaultService.totpParseUri(uri);
    },

    queryAuditLog: async (limit?: number) => {
      return vaultService.queryAuditLog(limit);
    },

    showUnlockWindow: async () => {
      await vaultService.showUnlockWindow();
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
      set({ hibpAuditing: true });
      try {
        const leaks = await vaultService.hibpAudit();
        const leakMap: Record<string, number> = {};
        for (const [id, count] of leaks) {
          leakMap[id] = count;
        }
        set({ hibpLeaks: leakMap, hibpAuditing: false });
      } catch (e) {
        log.error("hibpAudit failed", e);
        set({ hibpAuditing: false });
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
