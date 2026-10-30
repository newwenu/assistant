export interface AppConf {
  theme: string;
  language: string;
  hotkey: string;
  stayOnTop: boolean;
  launchAtLogin: boolean;
  closeToTray: boolean;
  keybindings: Record<string, string>;
  autoCompact: boolean;
  compactThresholdPercent: number;
  compactSummaryPrompt: string | null;
  compactSummaryMaxTokens: number;
}

export interface VaultConf {
  vaultAutoLockMinutes: number;
  vaultLockOnScreenLock: boolean;
  vaultClipboardClearSeconds: number;
  vaultCipherAlgorithm: "aes256Gcm" | "chacha20Poly1305";
  vaultBiometricEnabled: boolean;
  vaultHibpAutoAudit: boolean;
  vaultSyncRemoteUrl: string | null;
  vaultSshAutoAdd: boolean;
  vaultSshUsePageant: boolean;
  vaultSshUseOpensshAgent: boolean;
  vaultSshAuthSockOverride: string;
  vaultSyncLastAt: number | null;
  vaultSyncSshHosts: string[];
}

export interface OrbConf {
  clickThrough: boolean;
  alwaysOnTop: boolean;
  scalePercent: number;
  opacityPercent: number;
  hideOnPointerHover: boolean;
  hoverDelayMs: number;
  idleOpacityPercent: number;
  positionX: number | null;
  positionY: number | null;
}

export interface MascotConf {
  renderer: string;
  primaryColor: string | null;
  secondaryColor: string | null;
  riveIdlePoseRotation: boolean;
  riveSrc: string | null;
  svgAnimated: boolean;
  gifSrc: string | null;
  gifBorderRadius: number;
}
