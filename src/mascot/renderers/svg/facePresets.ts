export interface FacePreset {
  eyeScaleY: number;
  eyeScaleX: number;
  browTilt: number;
  browDy: number;
  showBrows: boolean;
  blushOpacity: number;
}

/** 表情预设表：眼睛缩放、眉毛倾角与腮红浓度，动画只叠加眨眼与浮动。 */
export const FACE_PRESETS: Record<string, FacePreset> = {
  sleep: {
    eyeScaleY: 0.1,
    eyeScaleX: 1,
    browTilt: 0,
    browDy: 2,
    showBrows: false,
    blushOpacity: 0.5,
  },
  idle: {
    eyeScaleY: 1,
    eyeScaleX: 1,
    browTilt: 0,
    browDy: 0,
    showBrows: false,
    blushOpacity: 0.85,
  },
  listening: {
    eyeScaleY: 1.05,
    eyeScaleX: 1.05,
    browTilt: -8,
    browDy: -10,
    showBrows: true,
    blushOpacity: 0.9,
  },
  thinking: {
    eyeScaleY: 0.7,
    eyeScaleX: 1,
    browTilt: -4,
    browDy: -2,
    showBrows: true,
    blushOpacity: 0.6,
  },
  confused: {
    eyeScaleY: 0.85,
    eyeScaleX: 0.95,
    browTilt: 14,
    browDy: -4,
    showBrows: true,
    blushOpacity: 0.55,
  },
  speaking: {
    eyeScaleY: 1,
    eyeScaleX: 1,
    browTilt: 0,
    browDy: 0,
    showBrows: false,
    blushOpacity: 0.95,
  },
  happy: {
    eyeScaleY: 0.45,
    eyeScaleX: 1.1,
    browTilt: -6,
    browDy: -6,
    showBrows: false,
    blushOpacity: 1,
  },
  concerned: {
    eyeScaleY: 0.95,
    eyeScaleX: 0.95,
    browTilt: 22,
    browDy: -2,
    showBrows: true,
    blushOpacity: 0.5,
  },
  curious: {
    eyeScaleY: 1.1,
    eyeScaleX: 1.05,
    browTilt: -10,
    browDy: -8,
    showBrows: true,
    blushOpacity: 0.8,
  },
  proud: {
    eyeScaleY: 0.55,
    eyeScaleX: 1.15,
    browTilt: -4,
    browDy: -4,
    showBrows: false,
    blushOpacity: 1,
  },
  cautious: {
    eyeScaleY: 0.9,
    eyeScaleX: 0.95,
    browTilt: 10,
    browDy: -3,
    showBrows: true,
    blushOpacity: 0.65,
  },
  celebrating: {
    eyeScaleY: 0.4,
    eyeScaleX: 1.15,
    browTilt: -8,
    browDy: -8,
    showBrows: false,
    blushOpacity: 1,
  },
  writing: {
    eyeScaleY: 0.75,
    eyeScaleX: 1,
    browTilt: -2,
    browDy: -1,
    showBrows: false,
    blushOpacity: 0.7,
  },
  reading: {
    eyeScaleY: 0.85,
    eyeScaleX: 1.05,
    browTilt: -6,
    browDy: -4,
    showBrows: true,
    blushOpacity: 0.75,
  },
  waving: {
    eyeScaleY: 0.5,
    eyeScaleX: 1.1,
    browTilt: -6,
    browDy: -6,
    showBrows: false,
    blushOpacity: 1,
  },
  dancing: {
    eyeScaleY: 0.4,
    eyeScaleX: 1.15,
    browTilt: -8,
    browDy: -8,
    showBrows: false,
    blushOpacity: 1,
  },
  working: {
    eyeScaleY: 0.75,
    eyeScaleX: 1,
    browTilt: -2,
    browDy: -1,
    showBrows: false,
    blushOpacity: 0.7,
  },
};
