import { createLogger } from "@/services/logger";
import { vaultService } from "../vault.service";

const log = createLogger("vaultStore");

const BIOMETRIC_CACHE_TTL_MS = 5_000;

export interface VaultBiometricSlice {
  biometricAvailable: boolean;
  biometricEnrolled: boolean;
  _biometricAvailableAt: number;
  _biometricEnrolledAt: number;

  checkBiometricAvailable: () => Promise<void>;
  checkBiometricEnrolled: () => Promise<void>;
  unlockBiometric: () => Promise<boolean>;
  enrollBiometric: (masterPassword: string) => Promise<void>;
  disenrollBiometric: () => Promise<void>;
}

type FullState = VaultBiometricSlice & {
  isLocked: boolean;
  loading: boolean;
  error: string | null;
  resetAutoLockTimer: () => void;
};

export function createBiometricSlice(
  set: (
    partial: Partial<FullState> | ((state: FullState) => Partial<FullState>),
  ) => void,
  get: () => FullState,
): VaultBiometricSlice {
  return {
    biometricAvailable: false,
    biometricEnrolled: false,
    _biometricAvailableAt: 0,
    _biometricEnrolledAt: 0,

    checkBiometricAvailable: async () => {
      const now = Date.now();
      if (now - get()._biometricAvailableAt < BIOMETRIC_CACHE_TTL_MS) return;
      try {
        const available = await vaultService.isBiometricAvailable();
        set({
          biometricAvailable: available,
          _biometricAvailableAt: Date.now(),
        });
        log.debug(`Biometric available: ${available}`);
      } catch (e) {
        log.error("checkBiometricAvailable failed", e);
        set({ biometricAvailable: false, _biometricAvailableAt: Date.now() });
      }
    },

    checkBiometricEnrolled: async () => {
      const now = Date.now();
      if (now - get()._biometricEnrolledAt < BIOMETRIC_CACHE_TTL_MS) return;
      try {
        const enrolled = await vaultService.isBiometricEnrolled();
        set({ biometricEnrolled: enrolled, _biometricEnrolledAt: Date.now() });
        log.debug(`Biometric enrolled: ${enrolled}`);
      } catch (e) {
        log.error("checkBiometricEnrolled failed", e);
        set({ biometricEnrolled: false, _biometricEnrolledAt: Date.now() });
      }
    },

    unlockBiometric: async () => {
      set({ loading: true, error: null });
      try {
        const ok = await vaultService.unlockBiometric();
        if (ok) {
          set({ isLocked: false, loading: false, _biometricEnrolledAt: 0 });
          get().resetAutoLockTimer();
          log.info("Vault unlocked via biometric");
        } else {
          set({ loading: false });
          log.warn("Biometric unlock cancelled or failed");
        }
        return ok;
      } catch (e) {
        const msg = String(e);
        if (msg.includes("NotFound") || msg.includes("Crypto")) {
          try {
            await vaultService.disenrollBiometric();
            set({ biometricEnrolled: false, _biometricEnrolledAt: 0 });
          } catch {
            log.warn(
              "disenrollBiometric also failed during unlockBiometric cleanup",
            );
          }
        }
        log.error("unlockBiometric failed", e);
        set({ error: msg, loading: false });
        return false;
      }
    },

    enrollBiometric: async (masterPassword) => {
      set({ loading: true, error: null });
      try {
        await vaultService.enrollBiometric(masterPassword);
        set({
          biometricEnrolled: true,
          loading: false,
          _biometricEnrolledAt: 0,
        });
        log.info("Biometric enrolled successfully");
      } catch (e) {
        log.error("enrollBiometric failed", e);
        set({ error: String(e), loading: false });
      }
    },

    disenrollBiometric: async () => {
      try {
        await vaultService.disenrollBiometric();
        set({ biometricEnrolled: false, _biometricEnrolledAt: 0 });
        log.info("Biometric disenrolled");
      } catch (e) {
        log.error("disenrollBiometric failed", e);
      }
    },
  };
}
