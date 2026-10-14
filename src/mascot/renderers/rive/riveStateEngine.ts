import type { MascotFace } from "../../core/types";
import { RIVE_VISEME_SET } from "./riveMaps";

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
  states: Record<string, string> & { idle: string; thinking: string };
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

export function resolveFaceToPose(
  face: MascotFace,
  engine: RiveStateEngine,
): string {
  const mapped = engine.states[face];
  if (mapped) return mapped;
  return engine.states.idle;
}
