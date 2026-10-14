import type { ChatFacePolicy, AckRule, ExpressionPolicy } from "./types";
import { asFace } from "./types";

export const DEFAULT_ACK_RULES: AckRule[] = [
  {
    pattern: /\b(sorry|failed|error|cannot|unable|problem)\b/i,
    face: asFace("concerned"),
    priority: 70,
  },
  {
    pattern: /\b(warning|careful|note|caution|might not|may not)\b/i,
    face: asFace("cautious"),
    priority: 60,
  },
  {
    pattern: /\b(congratulations|awesome|perfect|excellent|brilliant)\b/i,
    face: asFace("celebrating"),
    priority: 50,
    requiresToolCall: true,
  },
  {
    pattern: /\b(done|completed|fixed|success|ready|great)\b/i,
    face: asFace("proud"),
    priority: 45,
    requiresToolCall: true,
  },
  {
    pattern: /\b(done|completed|fixed|success|ready|great)\b/i,
    face: asFace("happy"),
    priority: 40,
  },
  {
    pattern: /\b(wrote|created|saved|updated file|edited)\b/i,
    face: asFace("writing"),
    priority: 30,
  },
  {
    pattern: /\b(read|found|searched|browsed|looked up)\b/i,
    face: asFace("reading"),
    priority: 20,
  },
];

export const DEFAULT_CHAT_FACE_POLICY: ChatFacePolicy = {
  onStreamStart: {
    reasoning: asFace("thinking"),
    toolCall: asFace("working"),
    default: asFace("speaking"),
  },
  onStreamEnd: asFace("writing"),
  onReasoningEnd: {
    toolCall: asFace("working"),
    default: asFace("speaking"),
  },
  onError: asFace("concerned"),
};

export const DEFAULT_EXPRESSION_POLICY: ExpressionPolicy = {
  chatFace: DEFAULT_CHAT_FACE_POLICY,
  ackRules: DEFAULT_ACK_RULES,
};