export type {
  MascotFace,
  MascotRendererId,
  RendererCapabilities,
  RendererMeta,
  RiveRendererConfig,
  SvgRendererConfig,
  GifRendererConfig,
  MascotRendererProps,
} from "./types";
export { asRendererId, BUILTIN_RENDERER_IDS } from "./types";
export { resolveVisemeCode, pickIdleFlourish, lerpViseme } from "./resolve";
export type { VisemeShape } from "./resolve";
export { registerRenderer, getRenderer, getAllRendererMetas } from "./registry";
export { hexToArgb, shadeHex, argbToHex } from "./colorUtils";
