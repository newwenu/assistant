export type MascotFace =
  | "sleep"
  | "idle"
  | "listening"
  | "thinking"
  | "confused"
  | "speaking"
  | "happy"
  | "concerned"
  | "curious"
  | "proud"
  | "cautious"
  | "celebrating"
  | "writing"
  | "reading"
  | "waving"
  | "dancing"
  | "working";

export type MascotRendererId = string & { __brand: "MascotRendererId" };

export function asRendererId(id: string): MascotRendererId {
  return id as MascotRendererId;
}

export const BUILTIN_RENDERER_IDS = {
  rive: asRendererId("rive"),
  svg: asRendererId("svg"),
  gif: asRendererId("gif"),
  disabled: asRendererId("disabled"),
} as const;

export interface RendererCapabilities {
  visemeSync: boolean;
  faceTransition: boolean;
  colorCustomization: boolean;
  idleAnimation: boolean;
}

export interface RendererMeta {
  id: MascotRendererId;
  label: string;
  capabilities: RendererCapabilities;
  limitations?: string;
  defaultColors?: { primary: string; secondary: string | null } | null;
}

export interface RiveRendererConfig {
  idlePoseRotation?: boolean;
  src?: string | null;
  buffer?: ArrayBuffer | null;
  stateMachine?: string | null;
}

export interface SvgRendererConfig {
  animated?: boolean;
  variant?: "shaded" | "flat";
}

export interface GifRendererConfig {
  src?: string | null;
  borderRadius?: number;
}

export interface MascotRendererProps {
  face: MascotFace;
  size?: number | string;
  visemeCode?: string;
  primaryColor?: number | null;
  secondaryColor?: number | null;
  rendererConfig?: Record<string, unknown>;
}
