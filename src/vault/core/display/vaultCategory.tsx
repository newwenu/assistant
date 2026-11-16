import type { ReactNode } from "react";
import {
  UserOutlined,
  FileTextOutlined,
  CreditCardOutlined,
  IdcardOutlined,
  CodeOutlined,
  ToolOutlined,
} from "@ant-design/icons";
import type { TFunction } from "i18next";

import type { VaultCategory } from "../vault.types";

const VAULT_CATEGORIES: readonly VaultCategory[] = [
  "login",
  "note",
  "card",
  "identity",
  "sshKey",
  "custom",
];

const VAULT_CATEGORY_ICONS: Record<VaultCategory, ReactNode> = {
  login: <UserOutlined />,
  note: <FileTextOutlined />,
  card: <CreditCardOutlined />,
  identity: <IdcardOutlined />,
  sshKey: <CodeOutlined />,
  custom: <ToolOutlined />,
};

export function isVaultCategory(value: string): value is VaultCategory {
  return VAULT_CATEGORIES.includes(value as VaultCategory);
}

export function vaultCategoryLabel(t: TFunction, value: string): string {
  return isVaultCategory(value) ? t(`vault.entry.category_${value}`) : value;
}

export function vaultCategoryIcon(value: string): ReactNode {
  return isVaultCategory(value)
    ? VAULT_CATEGORY_ICONS[value]
    : VAULT_CATEGORY_ICONS.custom;
}

export function vaultCategoryOptions(
  t: TFunction,
): { value: VaultCategory; label: string }[] {
  return VAULT_CATEGORIES.map((value) => ({
    value,
    label: vaultCategoryLabel(t, value),
  }));
}
