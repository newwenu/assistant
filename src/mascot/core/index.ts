export type {
  MascotFace,
  MascotRendererId,
  RendererCapabilities,
  RendererMeta,
  RendererSettingDef,
  RiveRendererConfig,
  SvgRendererConfig,
  GifRendererConfig,
  MascotRendererProps,
} from "./types";
export { asRendererId, BUILTIN_RENDERER_IDS } from "./types";
export { resolveVisemeCode, pickIdleFlourish, lerpViseme } from "./resolve";
export type { VisemeShape } from "./resolve";
export {
  registerRenderer,
  getRenderer,
  getAllRendererMetas,
  createMascotCapabilityVisible,
} from "./registry";
export { hexToArgb, shadeHex, argbToHex } from "./colorUtils";
