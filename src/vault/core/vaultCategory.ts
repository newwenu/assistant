import type { TFunction } from "i18next";

import type { VaultCategory } from "./vault.types";

const VAULT_CATEGORIES: readonly VaultCategory[] = [
  "login",
  "note",
  "card",
  "identity",
  "sshKey",
  "custom",
];

export function isVaultCategory(value: string): value is VaultCategory {
  return VAULT_CATEGORIES.includes(value as VaultCategory);
}

export function vaultCategoryLabel(t: TFunction, value: string): string {
  return isVaultCategory(value) ? t(`vault.entry.category_${value}`) : value;
}
