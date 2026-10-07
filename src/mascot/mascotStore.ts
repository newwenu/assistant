import { create } from "zustand";
import type { MascotFace } from "./faceStates";

const ACK_FACE_HOLD_MS = 700;
const VISEME_DECAY_MS = 180;

interface MascotState {
  face: MascotFace;
  visemeCode: string;
  setFace: (face: MascotFace) => void;
  setVisemeCode: (code: string) => void;
  holdThenIdle: (ackFace: MascotFace, ms?: number) => void;
}

let ackTimer: ReturnType<typeof setTimeout> | null = null;
let visemeDecayTimer: ReturnType<typeof setTimeout> | null = null;

function clearAckTimer() {
  if (ackTimer != null) {
    clearTimeout(ackTimer);
    ackTimer = null;
  }
}

function clearVisemeDecayTimer() {
  if (visemeDecayTimer != null) {
    clearTimeout(visemeDecayTimer);
    visemeDecayTimer = null;
  }
}

export const useMascotStore = create<MascotState>((set) => ({
  face: "idle",
  visemeCode: "sil",

  setFace: (face: MascotFace) => {
    clearAckTimer();
    set({ face });
  },

  setVisemeCode: (code: string) => {
    clearVisemeDecayTimer();
    set({ visemeCode: code });
    visemeDecayTimer = setTimeout(() => {
      visemeDecayTimer = null;
      set({ visemeCode: "sil" });
    }, VISEME_DECAY_MS);
  },

  holdThenIdle: (ackFace: MascotFace, ms = ACK_FACE_HOLD_MS) => {
    clearAckTimer();
    set({ face: ackFace });
    ackTimer = setTimeout(() => {
      ackTimer = null;
      set({ face: "idle" });
    }, ms);
  },
}));

export function deriveMascotFaceFromChat(
  isStreaming: boolean,
  isReasoningStreaming: boolean,
  hasError: boolean,
): MascotFace {
  if (hasError) return "concerned";
  if (isStreaming) return "speaking";
  if (isReasoningStreaming) return "thinking";
  return "idle";
}
