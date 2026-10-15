export { MascotRenderer } from "./MascotRenderer";
export {
  useMascotStore,
  attachEmotionEngine,
  detachEmotionEngine,
} from "./mascotStore";
export { useMascotChatSync } from "./useMascotChatSync";
export type {
  MascotFace,
  BuiltinFace,
  MascotRendererId,
  MascotRendererProps,
  RendererConfig,
  RendererMeta,
  RendererCapabilities,
  RendererSettingDef,
  RiveRendererConfig,
  SvgRendererConfig,
  GifRendererConfig,
  ChatFacePolicy,
  AckRule,
  ExpressionPolicy,
  EmotionEvent,
  EmotionEngine,
} from "./core/types";
export {
  asFace,
  asRendererId,
  BUILTIN_FACES,
  BUILTIN_RENDERER_IDS,
} from "./core/types";
export {
  DEFAULT_ACK_RULES,
  DEFAULT_CHAT_FACE_POLICY,
  DEFAULT_EXPRESSION_POLICY,
} from "./core/expressionPolicy";
export {
  getAllRendererMetas,
  getRenderer,
  createMascotCapabilityVisible,
} from "./core/registry";
export { hexToArgb, shadeHex, argbToHex } from "./core/colorUtils";
export type { VisemeShape } from "./core/resolve";
export {
  resolveVisemeCode,
  pickIdleFlourish,
  lerpViseme,
} from "./core/resolve";
