import type { ConfSlot } from "@/settings/core/types/confSlot";
import type { VaultConfDto } from "@/types/protocol.contract";

export type VaultConf = VaultConfDto;

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
