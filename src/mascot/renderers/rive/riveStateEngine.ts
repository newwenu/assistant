import type { MascotFace, MascotScene } from "../../core/types";
import {
  deriveFaceToPose,
  filterFaceToPose,
  filterSceneToPose,
} from "./riveIntrospect";
import { RIVE_POSES, RIVE_VISEME_SET } from "./riveMaps";

export interface RiveChannel {
  key: string;
  values: string[];
  default?: string;
  cycle?: {
    enabled: boolean;
    intervalMs?: number;
    order?: "random" | "sequential";
  };
}

export interface RiveStateEngine {
  visemeCodes: readonly string[];
  /** 表情 → pose（场景缺失或场景未声明时的兜底）。 */
  states: Record<string, string>;
  /** 场景 → pose；存在时优先于 `states`。 */
  sceneStates?: Record<string, string>;
  idlePoseCycle: string[];
  channels?: RiveChannel[];
}

export const BUILTIN_RIVE_STATE_ENGINE: RiveStateEngine = {
  visemeCodes: RIVE_VISEME_SET,
  states: {
    sleep: "idle",
    idle: "idle",
    listening: "idle",
    thinking: "thinking",
    confused: "thinking",
    speaking: "writing",
    happy: "celebration",
    concerned: "thinking",
    curious: "bookreading",
    proud: "celebration",
    cautious: "thinking",
    celebrating: "celebration",
    writing: "writing",
    reading: "bookreading",
    waving: "hand_wave",
    dancing: "dancing",
    working: "writing",
  },
  idlePoseCycle: [
    "thinking",
    "bookreading",
    "coffeedrink",
    "writing",
    "bobbateadrink",
    "dancing",
    "hand_wave",
    "celebration",
  ],
};

export interface RiveStateEngineOptions {
  poses?: readonly string[];
  visemes?: readonly string[];
  idlePoseCycle?: readonly string[];
  faceToPose?: Record<string, string>;
  sceneToPose?: Record<string, string>;
}

export function resolveFaceToPose(
  face: MascotFace,
  engine: RiveStateEngine,
): string {
  const mapped = engine.states[face];
  if (mapped) return mapped;
  return engine.states.idle ?? "idle";
}

/**
 * 场景优先：场景是运行时的真实触发源，表情只是它的中间产物。
 * 场景未声明或引擎里没有该场景时，回退到表情映射。
 */
export function resolveScenePose(
  scene: MascotScene | null | undefined,
  face: MascotFace,
  engine: RiveStateEngine,
): string {
  const mapped = scene ? engine.sceneStates?.[scene] : undefined;
  if (mapped) return mapped;
  return resolveFaceToPose(face, engine);
}

export function buildStateEngine(
  faceToPose?: Record<string, string>,
  options?: RiveStateEngineOptions,
): RiveStateEngine {
  if (!faceToPose && !options) return BUILTIN_RIVE_STATE_ENGINE;

  const poses = options?.poses?.length ? options.poses : RIVE_POSES;
  const visemeCodes = options?.visemes?.length
    ? options.visemes
    : RIVE_VISEME_SET;
  const introspected = options?.poses?.length ? options.poses : null;

  const states: Record<string, string> = {
    ...BUILTIN_RIVE_STATE_ENGINE.states,
    ...deriveFaceToPose(poses, BUILTIN_RIVE_STATE_ENGINE.states),
  };

  const declared = options?.faceToPose;
  if (declared) {
    const { kept } = introspected
      ? filterFaceToPose(declared, introspected)
      : { kept: declared };
    Object.assign(states, kept);
  }
  if (faceToPose) Object.assign(states, faceToPose);

  const declaredScenes = options?.sceneToPose;
  let sceneStates: Record<string, string> | undefined;
  if (declaredScenes) {
    const { kept } = introspected
      ? filterSceneToPose(declaredScenes, introspected)
      : { kept: declaredScenes };
    sceneStates = kept;
    const sceneIdle = kept.idle;
    if (sceneIdle) states.idle = sceneIdle;
  }

  const idlePose = states.idle || poses[0] || "idle";
  states.idle = idlePose;

  const idlePoseCycle = options?.idlePoseCycle?.length
    ? [...options.idlePoseCycle]
    : options?.poses?.length
      ? poses.filter((pose) => pose !== idlePose)
      : [...BUILTIN_RIVE_STATE_ENGINE.idlePoseCycle];

  return {
    visemeCodes,
    states,
    sceneStates,
    idlePoseCycle,
    channels: BUILTIN_RIVE_STATE_ENGINE.channels,
  };
}
