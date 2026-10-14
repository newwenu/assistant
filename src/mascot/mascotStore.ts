import { create } from "zustand";
import type { MascotFace, EmotionEngine } from "./core/types";
import { asFace } from "./core/types";

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
  face: asFace("idle"),
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
      set({ face: asFace("idle") });
    }, ms);
  },

  notifyListening: () => {
    const { face } = get();
    if (face !== asFace("idle") && face !== asFace("listening")) return;
    clearListeningTimer();
    set({ face: asFace("listening") });
    listeningTimer = setTimeout(() => {
      listeningTimer = null;
      set({ face: asFace("idle") });
    }, LISTENING_DEBOUNCE_MS);
  },
}));

let emotionUnsubscribe: (() => void) | null = null;

export function attachEmotionEngine(engine: EmotionEngine): void {
  detachEmotionEngine();
  emotionUnsubscribe = engine.subscribe((event) => {
    const { holdMs } = event;
    if (holdMs != null) {
      useMascotStore.getState().holdThenIdle(event.face, holdMs);
    } else {
      useMascotStore.getState().setFace(event.face);
    }
  });
}

export function detachEmotionEngine(): void {
  if (emotionUnsubscribe) {
    emotionUnsubscribe();
    emotionUnsubscribe = null;
  }
}
