import { create } from "zustand";
import type { MascotFace, MascotScene, EmotionEngine } from "./core/types";
import { asFace } from "./core/types";
import { sceneFace } from "./core/expressionPolicy";

const ACK_FACE_HOLD_MS = 700;
const VISEME_DECAY_MS = 180;
const LISTENING_DEBOUNCE_MS = 2_000;

interface MascotState {
  face: MascotFace;
  /**
   * 触发当前表情的场景。`null` 表示表情由外部直接设置（EmotionEngine），
   * 此时无法追溯到场景，按表情映射兜底。
   */
  scene: MascotScene | null;
  visemeCode: string;
  setFace: (face: MascotFace) => void;
  setScene: (scene: MascotScene) => void;
  setVisemeCode: (code: string) => void;
  holdSceneThenIdle: (scene: MascotScene, ms?: number) => void;
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
  scene: "idle",
  visemeCode: "sil",

  setFace: (face: MascotFace) => {
    clearAckTimer();
    clearListeningTimer();
    set({ face, scene: null });
  },

  setScene: (scene: MascotScene) => {
    clearAckTimer();
    clearListeningTimer();
    set({ scene, face: sceneFace(scene) });
  },

  setVisemeCode: (code: string) => {
    clearVisemeDecayTimer();
    set({ visemeCode: code });
    visemeDecayTimer = setTimeout(() => {
      visemeDecayTimer = null;
      set({ visemeCode: "sil" });
    }, VISEME_DECAY_MS);
  },

  holdSceneThenIdle: (scene: MascotScene, ms = ACK_FACE_HOLD_MS) => {
    clearAckTimer();
    clearListeningTimer();
    set({ scene, face: sceneFace(scene) });
    ackTimer = setTimeout(() => {
      ackTimer = null;
      set({ scene: "idle", face: sceneFace("idle") });
    }, ms);
  },

  holdThenIdle: (ackFace: MascotFace, ms = ACK_FACE_HOLD_MS) => {
    clearAckTimer();
    clearListeningTimer();
    set({ face: ackFace, scene: null });
    ackTimer = setTimeout(() => {
      ackTimer = null;
      set({ scene: "idle", face: sceneFace("idle") });
    }, ms);
  },

  notifyListening: () => {
    const { face } = get();
    if (face !== asFace("idle") && face !== asFace("listening")) return;
    clearListeningTimer();
    set({ scene: "listening", face: sceneFace("listening") });
    listeningTimer = setTimeout(() => {
      listeningTimer = null;
      set({ scene: "idle", face: sceneFace("idle") });
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
