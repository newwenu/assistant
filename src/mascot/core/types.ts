export type MascotFace = string & { __brand: "MascotFace" };

export function asFace(s: string): MascotFace {
  return s as MascotFace;
}

export const BUILTIN_FACES = [
  "sleep",
  "idle",
  "listening",
  "thinking",
  "confused",
  "speaking",
  "happy",
  "concerned",
  "curious",
  "proud",
  "cautious",
  "celebrating",
  "writing",
  "reading",
  "waving",
  "dancing",
  "working",
] as const;

export type BuiltinFace = (typeof BUILTIN_FACES)[number];

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
  settings?: RendererSettingDef[];
  supportedFaces?: MascotFace[];
}

export interface RendererSettingDef {
  key: string;
  type: string;
  default: unknown;
  label: string;
  description?: string;
  sideEffect?: string;
  min?: number;
  max?: number;
  step?: number;
  options?: { label: string; value: string }[];
  marks?: Record<number, string>;
  extra?: string;
  saveMessage?: string | false;
  assetConfig?: {
    assetType: string;
    fileFilter: { name: string; extensions: string[] };
    urlInput?: boolean;
  };
}

export interface RiveRendererConfig {
  idlePoseRotation?: boolean;
  src?: string | null;
  buffer?: ArrayBuffer | null;
  stateMachine?: string | null;
  faceToPose?: Record<string, string>;
}

export interface SvgRendererConfig {
  animated?: boolean;
  variant?: "shaded" | "flat";
}

export interface GifRendererConfig {
  src?: string | null;
  borderRadius?: number;
}

export type RendererConfig =
  RiveRendererConfig | SvgRendererConfig | GifRendererConfig;

export interface MascotRendererProps {
  face: MascotFace;
  size?: number | string;
  visemeCode?: string;
  primaryColor?: number | null;
  secondaryColor?: number | null;
  rendererConfig?: RendererConfig;
}

export interface ChatFacePolicy {
  onStreamStart: {
    reasoning: MascotFace;
    toolCall: MascotFace;
    default: MascotFace;
  };
  onStreamEnd: MascotFace;
  onReasoningEnd: { toolCall: MascotFace; default: MascotFace };
  onError: MascotFace;
}

export interface AckRule {
  pattern: RegExp;
  face: MascotFace;
  priority?: number;
  requiresToolCall?: boolean;
}

export interface ExpressionPolicy {
  chatFace: ChatFacePolicy;
  ackRules: AckRule[];
}

export interface EmotionEvent {
  face: MascotFace;
  holdMs?: number;
}

export interface EmotionEngine {
  subscribe: (listener: (event: EmotionEvent) => void) => () => void;
}
