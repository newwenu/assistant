export { MascotRenderer } from "./MascotRenderer";
export { useMascotStore, deriveMascotFaceFromChat } from "./mascotStore";
export { useMascotChatSync } from "./useMascotChatSync";
export type {
  MascotFace,
  MascotRendererId,
  MascotRendererProps,
  RendererMeta,
  RendererCapabilities,
  RiveRendererConfig,
  SvgRendererConfig,
  GifRendererConfig,
} from "./core/types";
export { asRendererId, BUILTIN_RENDERER_IDS } from "./core/types";
export { getAllRendererMetas, getRenderer } from "./core/registry";
export { hexToArgb, shadeHex, argbToHex } from "./core/colorUtils";
export type { VisemeShape } from "./core/resolve";
export {
  resolveVisemeCode,
  pickIdleFlourish,
  lerpViseme,
} from "./core/resolve";
