export interface AppConf {
  theme: string;
  language: string;
  hotkey: string;
  stayOnTop: boolean;
  launchAtLogin: boolean;
  closeToTray: boolean;
  keybindings: Record<string, string>;
  autoCompact: boolean;
  compactThresholdPercentOfBudget: number;
  compactSummaryPrompt: string | null;
  compactSummaryMaxTokens: number;
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
