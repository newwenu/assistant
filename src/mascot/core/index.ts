export type {
  MascotFace,
  BuiltinFace,
  MascotRendererId,
  RendererCapabilities,
  RendererMeta,
  RendererSettingDef,
  RiveRendererConfig,
  SvgRendererConfig,
  GifRendererConfig,
  MascotRendererProps,
  ChatFacePolicy,
  AckRule,
  ExpressionPolicy,
  EmotionEvent,
  EmotionEngine,
} from "./types";
export {
  asFace,
  asRendererId,
  BUILTIN_FACES,
  BUILTIN_RENDERER_IDS,
} from "./types";
export {
  DEFAULT_ACK_RULES,
  DEFAULT_CHAT_FACE_POLICY,
  DEFAULT_EXPRESSION_POLICY,
} from "./expressionPolicy";
export { resolveVisemeCode, pickIdleFlourish, lerpViseme } from "./resolve";
export type { VisemeShape } from "./resolve";
export {
  registerRenderer,
  getRenderer,
  getAllRendererMetas,
  createMascotCapabilityVisible,
} from "./registry";
export { hexToArgb, shadeHex, argbToHex } from "./colorUtils";
