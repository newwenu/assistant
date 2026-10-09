import { create } from "zustand";
import type { MascotFace } from "./faceStates";

const ACK_FACE_HOLD_MS = 700;
const VISEME_DECAY_MS = 180;
const LISTENING_DEBOUNCE_MS = 2_000;

interface MascotState {
  face: MascotFace;
  visemeCode: string;
  setFace: (face: MascotFace) => void;
  setVisemeCode: (code: string) => void;
  holdThenIdle: (ackFace: MascotFace, ms?: number) => void;
  notifyListening: () => void;
}

let ackTimer: ReturnType<typeof setTimeout> | null = null;
let visemeDecayTimer: ReturnType<typeof setTimeout> | null = null;
let listeningTimer: ReturnType<typeof setTimeout> | null = null;

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

function clearListeningTimer() {
  if (listeningTimer != null) {
    clearTimeout(listeningTimer);
    listeningTimer = null;
  }
}

export const useMascotStore = create<MascotState>((set, get) => ({
  face: "idle",
  visemeCode: "sil",

  setFace: (face: MascotFace) => {
    clearAckTimer();
    clearListeningTimer();
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
    clearListeningTimer();
    set({ face: ackFace });
    ackTimer = setTimeout(() => {
      ackTimer = null;
      set({ face: "idle" });
    }, ms);
  },

  notifyListening: () => {
    const { face } = get();
    if (face !== "idle" && face !== "listening") return;
    clearListeningTimer();
    set({ face: "listening" });
    listeningTimer = setTimeout(() => {
      listeningTimer = null;
      set({ face: "idle" });
    }, LISTENING_DEBOUNCE_MS);
  },
}));

export function deriveMascotFaceFromChat(
  isStreaming: boolean,
  isReasoningStreaming: boolean,
  hasError: boolean,
  isListening?: boolean,
  isWorking?: boolean,
): MascotFace {
  if (hasError) return "concerned";
  if (isReasoningStreaming) return "thinking";
  if (isWorking) return "working";
  if (isStreaming) return "speaking";
  if (isListening) return "listening";
  return "idle";
}