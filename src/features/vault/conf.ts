import type { ConfSlot } from "@/settings/core/types/confSlot";
import type { VaultConf } from "@/vault/core/vault.types";

declare module "@/settings/core/types/confRegistry" {
  interface ConfTypeRegistry {
    vault: VaultConf;
  }
}

export const vaultConfSlot = {
  group: "vault",
  invoke: "vault_get_conf",
  namespace: "vault",
  pageId: "vault",
} satisfies ConfSlot<"vault">;
