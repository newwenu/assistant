import type { ConfSlot } from "@/settings/core/types/confSlot";

export interface VaultConf {
  vaultAutoLockMinutes: number;
  vaultLockOnScreenLock: boolean;
  vaultClipboardClearSeconds: number;
  vaultCipherAlgorithm: "aes256Gcm" | "chacha20Poly1305";
  vaultHibpAutoAudit: boolean;
  vaultSyncRemoteUrl: string | null;
  vaultSshAutoAdd: boolean;
  vaultSshUsePageant: boolean;
  vaultSshUseOpensshAgent: boolean;
  vaultSshAuthSockOverride: string;
  vaultSyncLastAt: number | null;
  vaultSyncSshHosts: string[];
}

declare module "@/settings/core/types/confRegistry" {
  interface ConfTypeRegistry {
    vault: VaultConf;
  }
}

export const vaultConfSlot = {
  group: "vault",
  invoke: "get_vault_conf",
  pageId: "vault",
} satisfies ConfSlot<"vault">;
