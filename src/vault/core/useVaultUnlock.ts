import { useState, useCallback } from "react";
import { useVaultStore } from "./vaultStore";
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
    clearError,
    unlockBiometric,
    checkRecoveryKeySet,
    checkBiometricAvailable,
    checkBiometricEnrolled,
    biometricAvailable,
    biometricEnrolled,
    recoveryKeySet,
    disasterRecoveryStep,
    disasterRecoveryResult,
    startDisasterRecovery,
    cancelDisasterRecovery,
    setDisasterRecoveryStep,
    disasterRecoveryExport,
  } = useVaultStore();

  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [strength, setStrength] = useState<StrengthResult | null>(null);
  const [unlockMethod, setUnlockMethod] = useState<
    "password" | "biometric" | null
  >(null);
  const [drGitUrl, setDrGitUrl] = useState("");
  const [drRecoveryKey, setDrRecoveryKey] = useState("");
  const [drCommit, setDrCommit] = useState("");

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
    clearError,
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
    recoveryKeySet,
    handlePasswordSubmit,
    handleBiometricUnlock,
    checkBiometricAvailable,
    checkBiometricEnrolled,
    checkRecoveryKeySet,
    disasterRecoveryStep,
    disasterRecoveryResult,
    startDisasterRecovery,
    cancelDisasterRecovery,
    setDisasterRecoveryStep,
    disasterRecoveryExport,
    drGitUrl,
    setDrGitUrl,
    drRecoveryKey,
    setDrRecoveryKey,
    drCommit,
    setDrCommit,
  };
}
