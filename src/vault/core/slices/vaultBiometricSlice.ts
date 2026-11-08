import { createLogger } from "@/services/logger";
import { vaultService } from "../vault.service";
import { vaultErrorCode, vaultErrorMessage } from "../vaultError";
import type { VaultErrorCode } from "../vault.types";

const log = createLogger("vaultStore");

const BIOMETRIC_CACHE_TTL_MS = 5_000;
const BIOMETRIC_RETRY_BASE_MS = 250;
const BIOMETRIC_RETRY_MAX_MS = 4_000;
const BIOMETRIC_MAX_RETRIES = 5;

export interface VaultBiometricSlice {
  biometricAvailable: boolean;
  biometricEnrolled: boolean;
  _biometricAvailableAt: number;
  _biometricEnrolledAt: number;
  _biometricRetryTimerId: ReturnType<typeof setTimeout> | null;
  _biometricAvailRetryCount: number;
  _biometricEnrolledRetryCount: number;

  checkBiometricAvailable: () => Promise<void>;
  checkBiometricEnrolled: () => Promise<void>;
  unlockBiometric: () => Promise<void>;
  enrollBiometric: () => Promise<void>;
  disenrollBiometric: () => Promise<void>;
}

type FullState = VaultBiometricSlice & {
  isLocked: boolean;
  loading: boolean;
  error: string | null;
  errorCode: VaultErrorCode | null;
  resetAutoLockTimer: () => void;
};

export function createBiometricSlice(
  set: (
    partial: Partial<FullState> | ((state: FullState) => Partial<FullState>),
  ) => void,
  get: () => FullState,
): VaultBiometricSlice {
  const scheduleRetry = (): void => {
    const state = get();
    if (state._biometricRetryTimerId) return;
    const attempts = Math.max(
      state._biometricAvailRetryCount,
      state._biometricEnrolledRetryCount,
    );
    if (attempts > BIOMETRIC_MAX_RETRIES) return;
    const delay = Math.min(
      BIOMETRIC_RETRY_BASE_MS * 2 ** (attempts - 1),
      BIOMETRIC_RETRY_MAX_MS,
    );
    const id = setTimeout(() => {
      set({ _biometricRetryTimerId: null });
      void get().checkBiometricAvailable();
      void get().checkBiometricEnrolled();
    }, delay);
    set({ _biometricRetryTimerId: id });
  };

  return {
    biometricAvailable: false,
    biometricEnrolled: false,
    _biometricAvailableAt: 0,
    _biometricEnrolledAt: 0,
    _biometricRetryTimerId: null,
    _biometricAvailRetryCount: 0,
    _biometricEnrolledRetryCount: 0,

    checkBiometricAvailable: async () => {
      const now = Date.now();
      if (now - get()._biometricAvailableAt < BIOMETRIC_CACHE_TTL_MS) return;
      try {
        const available = await vaultService.isBiometricAvailable();
        set({
          biometricAvailable: available,
          _biometricAvailableAt: Date.now(),
          _biometricAvailRetryCount: 0,
        });
        log.debug(`Biometric available: ${available}`);
      } catch (e) {
        log.error("checkBiometricAvailable failed", e);
        set({
          biometricAvailable: false,
          _biometricAvailRetryCount: get()._biometricAvailRetryCount + 1,
        });
        scheduleRetry();
      }
    },

    checkBiometricEnrolled: async () => {
      const now = Date.now();
      if (now - get()._biometricEnrolledAt < BIOMETRIC_CACHE_TTL_MS) return;
      try {
        const enrolled = await vaultService.isBiometricEnrolled();
        set({
          biometricEnrolled: enrolled,
          _biometricEnrolledAt: Date.now(),
          _biometricEnrolledRetryCount: 0,
        });
        log.debug(`Biometric enrolled: ${enrolled}`);
      } catch (e) {
        log.error("checkBiometricEnrolled failed", e);
        set({
          biometricEnrolled: false,
          _biometricEnrolledRetryCount: get()._biometricEnrolledRetryCount + 1,
        });
        scheduleRetry();
      }
    },

    unlockBiometric: async () => {
      set({ loading: true, error: null, errorCode: null });
      try {
        await vaultService.unlockBiometric();
        set({ isLocked: false, loading: false, _biometricEnrolledAt: 0 });
        get().resetAutoLockTimer();
        log.info("Vault unlocked via biometric");
      } catch (e) {
        const code = vaultErrorCode(e);
        if (code === "crypto" || code === "notFound") {
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
        set({
          error: vaultErrorMessage(e),
          errorCode: code,
          loading: false,
        });
      }
    },

    enrollBiometric: async () => {
      set({ loading: true, error: null, errorCode: null });
      try {
        await vaultService.enrollBiometric();
        set({
          biometricEnrolled: true,
          loading: false,
          _biometricEnrolledAt: 0,
        });
        log.info("Biometric enrolled successfully");
      } catch (e) {
        log.error("enrollBiometric failed", e);
        set({
          error: vaultErrorMessage(e),
          errorCode: vaultErrorCode(e),
          loading: false,
        });
        throw e;
      }
    },

    disenrollBiometric: async () => {
      try {
        await vaultService.disenrollBiometric();
        set({ biometricEnrolled: false, _biometricEnrolledAt: 0 });
        log.info("Biometric disenrolled");
      } catch (e) {
        log.error("disenrollBiometric failed", e);
        throw e;
      }
    },
  };
}
