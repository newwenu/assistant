import { SafetyCertificateOutlined } from "@ant-design/icons";
import { vaultConfSlot } from "./conf";
import type { FeatureModule } from "../types";
import type { SettingSectionDef } from "@/settings/core/field/fieldRegistry";
import type { ExtensionSettingDefinition } from "@/settings/core/types/extension";

const VAULT_GENERAL_SETTINGS: ExtensionSettingDefinition[] = [
  {
    key: "vaultAutoLockMinutes",
    group: "vault",
    section: "vault-general",
    type: "number",
    default: 5,
    label: "settings.vault.auto_lock_minutes.label",
    min: 1,
    max: 60,
  },
  {
    key: "vaultLockOnScreenLock",
    group: "vault",
    section: "vault-general",
    type: "boolean",
    default: true,
    label: "settings.vault.lock_on_screen_lock.label",
  },
  {
    key: "vaultClipboardClearSeconds",
    group: "vault",
    section: "vault-general",
    type: "number",
    default: 30,
    label: "settings.vault.clipboard_clear_seconds.label",
    min: 5,
    max: 300,
  },
  {
    key: "vaultCipherAlgorithm",
    group: "vault",
    section: "vault-general",
    type: "select",
    default: "aes256Gcm",
    label: "settings.vault.cipher_algorithm.label",
    options: [
      { label: "AES-256-GCM", value: "aes256Gcm" },
      { label: "ChaCha20-Poly1305", value: "chacha20Poly1305" },
    ],
  },
  {
    key: "vaultBiometricEnabled",
    group: "vault",
    section: "vault-general",
    type: "boolean",
    default: false,
    label: "settings.vault.biometric_enabled.label",
    description: "settings.vault.biometric_enabled.description",
    customRenderLoad: () =>
      import("./settings/BiometricSwitchRender").then((m) => ({
        default: m.BiometricSwitchRender,
      })),
    saveMessage: "settings.vault.biometric_enabled.save_message",
  },
  {
    key: "vaultHibpAutoAudit",
    group: "vault",
    section: "vault-general",
    type: "boolean",
    default: true,
    label: "settings.vault.hibp_auto_audit.label",
  },
];

const VAULT_SSH_SETTINGS: ExtensionSettingDefinition[] = [
  {
    key: "vaultSshAutoAdd",
    group: "vault",
    section: "vault-ssh",
    type: "boolean",
    default: false,
    label: "settings.vault.ssh_auto_add.label",
  },
  {
    key: "vaultSshUsePageant",
    group: "vault",
    section: "vault-ssh",
    type: "boolean",
    default: true,
    label: "settings.vault.ssh_use_pageant.label",
    visible: () => navigator.platform.toLowerCase().includes("win"),
  },
  {
    key: "vaultSshUseOpensshAgent",
    group: "vault",
    section: "vault-ssh",
    type: "boolean",
    default: true,
    label: "settings.vault.ssh_use_openssh_agent.label",
    visible: () => navigator.platform.toLowerCase().includes("win"),
  },
  {
    key: "vaultSshAuthSockOverride",
    group: "vault",
    section: "vault-ssh",
    type: "string",
    default: "",
    label: "settings.vault.ssh_auth_sock_override.label",
    placeholder: "settings.vault.ssh_auth_sock_override.placeholder",
    visible: () => !navigator.platform.toLowerCase().includes("win"),
  },
];

const VAULT_SECTIONS: SettingSectionDef[] = [
  {
    id: "vault-general",
    pageId: "vault",
    title: "settings.vault.general.title",
    keywords: [
      "auto",
      "lock",
      "clipboard",
      "cipher",
      "biometric",
      "hibp",
      "自动锁定",
      "剪贴板",
      "加密",
      "生物识别",
    ],
  },
  {
    id: "vault-ssh",
    pageId: "vault",
    title: "settings.vault.ssh.title",
    keywords: ["ssh", "agent", "pageant", "openssh", "密钥"],
  },
];

export const vaultModule: FeatureModule = {
  id: "vault",
  order: 65,
  ipc: { namespace: "vault" },
  routes: [
    {
      path: "/vault",
      load: () => import("@/vault").then((m) => ({ default: m.VaultApp })),
      boundary: "密码保险库",
      navParent: "vault",
      keywords: ["vault", "password", "密码", "保险库"],
    },
  ],
  sidebar: [
    {
      key: "vault",
      label: "common.navigation.vault",
      icon: SafetyCertificateOutlined,
      slot: "after-tools",
    },
  ],
  categories: [
    { id: "vault", label: "common.navigation.category.vault", order: 20 },
  ],
  settings: {
    pages: [
      {
        id: "vault",
        categoryId: "vault",
        label: "common.navigation.vault_settings.title",
        keywords: [
          "vault",
          "password",
          "ssh",
          "biometric",
          "cipher",
          "密码",
          "保险库",
          "加密",
        ],
        icon: SafetyCertificateOutlined,
        order: 65,
        load: () =>
          import("./settings/VaultSettingsPage").then((m) => ({
            default: m.VaultSettingsPage,
          })),
      },
    ],
    sections: VAULT_SECTIONS,
    fields: [...VAULT_GENERAL_SETTINGS, ...VAULT_SSH_SETTINGS],
  },
  confs: [vaultConfSlot],
};
