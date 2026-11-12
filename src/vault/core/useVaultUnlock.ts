import { useState, useCallback } from "react";
import { useVaultShallow } from "./vaultStore";
import { vaultService } from "./vault.service";
import type { StrengthResult } from "./vault.types";

const MIN_SCORE = 2;

export function useVaultUnlock() {
  const {
    isInitialized,
    init,
    unlock,
    loading,
    error,
    errorCode,
    clearError,
    resetRepository,
    rebuildLocalDb,
    checkStatus,
    unlockBiometric,
    checkBiometricAvailable,
    checkBiometricEnrolled,
    biometricAvailable,
    biometricEnrolled,
  } = useVaultShallow((s) => ({
    isInitialized: s.isInitialized,
    init: s.init,
    unlock: s.unlock,
    loading: s.loading,
    error: s.error,
    errorCode: s.errorCode,
    clearError: s.clearError,
    resetRepository: s.resetRepository,
    rebuildLocalDb: s.rebuildLocalDb,
    checkStatus: s.checkStatus,
    unlockBiometric: s.unlockBiometric,
    checkBiometricAvailable: s.checkBiometricAvailable,
    checkBiometricEnrolled: s.checkBiometricEnrolled,
    biometricAvailable: s.biometricAvailable,
    biometricEnrolled: s.biometricEnrolled,
  }));

  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [strength, setStrength] = useState<StrengthResult | null>(null);
  const [unlockMethod, setUnlockMethod] = useState<
    "password" | "biometric" | null
  >(null);

  const canSubmit =
    !loading &&
    password.length > 0 &&
    (isInitialized ||
      (password === confirmPassword && (strength?.score ?? 0) >= MIN_SCORE));

  const handlePasswordSubmit = useCallback(async () => {
    clearError();
    if (!password) return;

    if (!isInitialized) {
      if (password !== confirmPassword) return;
      if (password.length < 8) return;
    }

    setUnlockMethod("password");
    try {
      if (!isInitialized) {
        await init(password);
      } else {
        await unlock(password);
      }
    } finally {
      setUnlockMethod(null);
    }
    setPassword("");
    setConfirmPassword("");
    setStrength(null);
  }, [password, confirmPassword, isInitialized, clearError, init, unlock]);

  const handleBiometricUnlock = useCallback(async () => {
    clearError();
    setUnlockMethod("biometric");
    try {
      await unlockBiometric();
    } finally {
      setUnlockMethod(null);
    }
  }, [clearError, unlockBiometric]);

  const checkStrength = useCallback(
    (pwd: string) => {
      if (!isInitialized && pwd.length > 0) {
        void vaultService.checkMasterPasswordStrength(pwd).then((result) => {
          setStrength(result);
        });
      } else {
        setStrength(null);
      }
    },
    [isInitialized],
  );

  return {
    isInitialized,
    loading,
    error,
    errorCode,
    clearError,
    resetRepository,
    rebuildLocalDb,
    checkStatus,
    password,
    setPassword,
    confirmPassword,
    setConfirmPassword,
    strength,
    setStrength,
    checkStrength,
    unlockMethod,
    canSubmit,
    biometricAvailable,
    biometricEnrolled,
    handlePasswordSubmit,
    handleBiometricUnlock,
    checkBiometricAvailable,
    checkBiometricEnrolled,
  };
}
