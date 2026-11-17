import type { I18nPath } from "@/i18n";
import type {
  AckRule,
  ChatScenePolicy,
  ExpressionPolicy,
  MascotFace,
  MascotScene,
  MascotSceneGroup,
} from "./types";
import { MASCOT_SCENES, asFace } from "./types";

export interface MascotSceneDef {
  face: MascotFace;
  group: MascotSceneGroup;
  labelKey: I18nPath;
  hintKey: I18nPath;
}

/**
 * 场景是运行时唯一的触发源：对话流开始/结束、推理流结束、工具调用、错误。
 * 表情只是场景的中间产物，配置时应当面向场景而不是表情。
 * 默认表情与改动前的硬编码策略保持一致。
 */
export const MASCOT_SCENE_DEFS: Record<MascotScene, MascotSceneDef> = {
  idle: {
    face: asFace("idle"),
    group: "runtime",
    labelKey: "settings.mascot.scene.idle.label",
    hintKey: "settings.mascot.scene.idle.hint",
  },
  listening: {
    face: asFace("listening"),
    group: "runtime",
    labelKey: "settings.mascot.scene.listening.label",
    hintKey: "settings.mascot.scene.listening.hint",
  },
  streamStartReasoning: {
    face: asFace("thinking"),
    group: "runtime",
    labelKey: "settings.mascot.scene.stream_start_reasoning.label",
    hintKey: "settings.mascot.scene.stream_start_reasoning.hint",
  },
  streamStartToolCall: {
    face: asFace("working"),
    group: "runtime",
    labelKey: "settings.mascot.scene.stream_start_tool_call.label",
    hintKey: "settings.mascot.scene.stream_start_tool_call.hint",
  },
  streamStart: {
    face: asFace("speaking"),
    group: "runtime",
    labelKey: "settings.mascot.scene.stream_start.label",
    hintKey: "settings.mascot.scene.stream_start.hint",
  },
  reasoningEndToolCall: {
    face: asFace("working"),
    group: "runtime",
    labelKey: "settings.mascot.scene.reasoning_end_tool_call.label",
    hintKey: "settings.mascot.scene.reasoning_end_tool_call.hint",
  },
  reasoningEnd: {
    face: asFace("speaking"),
    group: "runtime",
    labelKey: "settings.mascot.scene.reasoning_end.label",
    hintKey: "settings.mascot.scene.reasoning_end.hint",
  },
  streamEnd: {
    face: asFace("writing"),
    group: "runtime",
    labelKey: "settings.mascot.scene.stream_end.label",
    hintKey: "settings.mascot.scene.stream_end.hint",
  },
  error: {
    face: asFace("concerned"),
    group: "runtime",
    labelKey: "settings.mascot.scene.error.label",
    hintKey: "settings.mascot.scene.error.hint",
  },
  ackError: {
    face: asFace("concerned"),
    group: "ack",
    labelKey: "settings.mascot.scene.ack_error.label",
    hintKey: "settings.mascot.scene.ack_error.hint",
  },
  ackWarning: {
    face: asFace("cautious"),
    group: "ack",
    labelKey: "settings.mascot.scene.ack_warning.label",
    hintKey: "settings.mascot.scene.ack_warning.hint",
  },
  ackPraise: {
    face: asFace("celebrating"),
    group: "ack",
    labelKey: "settings.mascot.scene.ack_praise.label",
    hintKey: "settings.mascot.scene.ack_praise.hint",
  },
  ackDoneTool: {
    face: asFace("proud"),
    group: "ack",
    labelKey: "settings.mascot.scene.ack_done_tool.label",
    hintKey: "settings.mascot.scene.ack_done_tool.hint",
  },
  ackDone: {
    face: asFace("happy"),
    group: "ack",
    labelKey: "settings.mascot.scene.ack_done.label",
    hintKey: "settings.mascot.scene.ack_done.hint",
  },
  ackWriting: {
    face: asFace("writing"),
    group: "ack",
    labelKey: "settings.mascot.scene.ack_writing.label",
    hintKey: "settings.mascot.scene.ack_writing.hint",
  },
  ackReading: {
    face: asFace("reading"),
    group: "ack",
    labelKey: "settings.mascot.scene.ack_reading.label",
    hintKey: "settings.mascot.scene.ack_reading.hint",
  },
};

export const MASCOT_SCENE_GROUPS: readonly MascotSceneGroup[] = [
  "runtime",
  "ack",
];

export const MASCOT_SCENE_GROUP_LABEL_KEYS: Record<MascotSceneGroup, I18nPath> =
  {
    runtime: "settings.mascot.scene_group.runtime",
    ack: "settings.mascot.scene_group.ack",
  };

export function isMascotScene(value: string): value is MascotScene {
  return (MASCOT_SCENES as readonly string[]).includes(value);
}

export function sceneFace(scene: MascotScene): MascotFace {
  return MASCOT_SCENE_DEFS[scene].face;
}

export function scenesInGroup(group: MascotSceneGroup): MascotScene[] {
  return MASCOT_SCENES.filter(
    (scene) => MASCOT_SCENE_DEFS[scene].group === group,
  );
}

export const DEFAULT_ACK_RULES: AckRule[] = [
  {
    pattern: /\b(sorry|failed|error|cannot|unable|problem)\b/i,
    scene: "ackError",
    priority: 70,
  },
  {
    pattern: /\b(warning|careful|note|caution|might not|may not)\b/i,
    scene: "ackWarning",
    priority: 60,
  },
  {
    pattern: /\b(congratulations|awesome|perfect|excellent|brilliant)\b/i,
    scene: "ackPraise",
    priority: 50,
    requiresToolCall: true,
  },
  {
    pattern: /\b(done|completed|fixed|success|ready|great)\b/i,
    scene: "ackDoneTool",
    priority: 45,
    requiresToolCall: true,
  },
  {
    pattern: /\b(done|completed|fixed|success|ready|great)\b/i,
    scene: "ackDone",
    priority: 40,
  },
  {
    pattern: /\b(wrote|created|saved|updated file|edited)\b/i,
    scene: "ackWriting",
    priority: 30,
  },
  {
    pattern: /\b(read|found|searched|browsed|looked up)\b/i,
    scene: "ackReading",
    priority: 20,
  },
];

export const DEFAULT_CHAT_SCENE_POLICY: ChatScenePolicy = {
  onStreamStart: {
    reasoning: "streamStartReasoning",
    toolCall: "streamStartToolCall",
    default: "streamStart",
  },
  onStreamEnd: "streamEnd",
  onReasoningEnd: {
    toolCall: "reasoningEndToolCall",
    default: "reasoningEnd",
  },
  onError: "error",
};

export const DEFAULT_EXPRESSION_POLICY: ExpressionPolicy = {
  chatScene: DEFAULT_CHAT_SCENE_POLICY,
  ackRules: DEFAULT_ACK_RULES,
};
