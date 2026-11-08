import { create } from "zustand";
import { useShallow } from "zustand/react/shallow";
import {
  createLifecycleSlice,
  type VaultLifecycleSlice,
} from "./slices/vaultLifecycleSlice";
import {
  createEntriesSlice,
  type VaultEntriesSlice,
} from "./slices/vaultEntriesSlice";
import {
  createRevealSlice,
  type VaultRevealSlice,
} from "./slices/vaultRevealSlice";
import {
  createAuditSlice,
  type VaultAuditSlice,
} from "./slices/vaultAuditSlice";
import {
  createBiometricSlice,
  type VaultBiometricSlice,
} from "./slices/vaultBiometricSlice";
import { createSyncSlice, type VaultSyncSlice } from "./slices/vaultSyncSlice";
import {
  createRecoverySlice,
  type VaultRecoverySlice,
} from "./slices/vaultRecoverySlice";

export type VaultState = VaultLifecycleSlice &
  VaultEntriesSlice &
  VaultRevealSlice &
  VaultAuditSlice &
  VaultBiometricSlice &
  VaultSyncSlice &
  VaultRecoverySlice;

export function useVaultShallow<T>(selector: (state: VaultState) => T): T {
  return useVaultStore(useShallow(selector));
}

export const useVaultStore = create<VaultState>()((set, get): VaultState => {
  const lifecycleSlice = createLifecycleSlice(set, get);
  const entriesSlice = createEntriesSlice(set, get);
  const revealSlice = createRevealSlice(set, get);
  const auditSlice = createAuditSlice(set, get);
  const biometricSlice = createBiometricSlice(set, get);
  const syncSlice = createSyncSlice(set, get);
  const recoverySlice = createRecoverySlice(set, get);
  return {
    ...lifecycleSlice,
    ...entriesSlice,
    ...revealSlice,
    ...auditSlice,
    ...biometricSlice,
    ...syncSlice,
    ...recoverySlice,
  };
});
