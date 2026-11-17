export { MascotRenderer } from "./MascotRenderer";
export {
  useMascotStore,
  attachEmotionEngine,
  detachEmotionEngine,
} from "./mascotStore";
export { useMascotChatSync } from "./useMascotChatSync";
export type {
  MascotFace,
  MascotScene,
  MascotSceneGroup,
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
  ChatScenePolicy,
  AckRule,
  ExpressionPolicy,
  EmotionEvent,
  EmotionEngine,
} from "./core/types";
export {
  asFace,
  asRendererId,
  BUILTIN_FACES,
  MASCOT_SCENES,
  BUILTIN_RENDERER_IDS,
} from "./core/types";
export {
  DEFAULT_ACK_RULES,
  DEFAULT_CHAT_SCENE_POLICY,
  DEFAULT_EXPRESSION_POLICY,
  MASCOT_SCENE_DEFS,
  MASCOT_SCENE_GROUPS,
  MASCOT_SCENE_GROUP_LABEL_KEYS,
  isMascotScene,
  sceneFace,
  scenesInGroup,
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
