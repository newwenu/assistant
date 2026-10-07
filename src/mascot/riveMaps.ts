import type { MascotFace } from "./faceStates";

export const MASCOT_STATE_MACHINE = "MascotSM";

export const RIVE_POSES = [
  "idle",
  "thinking",
  "celebration",
  "bookreading",
  "coffeedrink",
  "writing",
  "bobbateadrink",
  "hand_wave",
  "dancing",
] as const;
export type RivePose = (typeof RIVE_POSES)[number];

export const FACE_TO_POSE: Record<MascotFace, RivePose> = {
  idle: "idle",
  listening: "idle",
  thinking: "thinking",
  speaking: "idle",
  happy: "celebration",
  concerned: "thinking",
  curious: "bookreading",
  working: "writing",
  waving: "hand_wave",
};

export function faceToPose(face: MascotFace): RivePose {
  return FACE_TO_POSE[face] ?? "idle";
}

export const RIVE_VISEME_SET = [
  "sil",
  "PP",
  "FF",
  "TH",
  "DD",
  "kk",
  "CH",
  "SS",
  "nn",
  "RR",
  "aa",
  "E",
  "ih",
  "oh",
  "ou",
] as const;
type RiveVisemeCode = (typeof RIVE_VISEME_SET)[number];

const VISEME_ALIAS: Record<string, RiveVisemeCode> = {
  sil: "sil",
  silence: "sil",
  pp: "PP",
  m: "PP",
  b: "PP",
  p: "PP",
  ff: "FF",
  f: "FF",
  v: "FF",
  th: "TH",
  dd: "DD",
  d: "DD",
  t: "DD",
  l: "DD",
  kk: "kk",
  k: "kk",
  g: "kk",
  ch: "CH",
  ss: "SS",
  s: "SS",
  z: "SS",
  nn: "nn",
  n: "nn",
  rr: "RR",
  r: "RR",
  aa: "aa",
  a: "aa",
  e: "E",
  ih: "ih",
  i: "ih",
  y: "ih",
  oh: "oh",
  o: "oh",
  ou: "ou",
  u: "ou",
  w: "ou",
};

export function toRiveVisemeCode(code: string): RiveVisemeCode {
  return VISEME_ALIAS[code.toLowerCase()] ?? "sil";
}

export const AMBIENT_POSES: readonly RivePose[] = [
  "thinking",
  "bookreading",
  "coffeedrink",
  "writing",
  "bobbateadrink",
  "dancing",
  "hand_wave",
  "celebration",
];

export function pickAmbientPose(
  exclude?: RivePose,
  rng: () => number = Math.random,
): RivePose {
  const pool = exclude
    ? AMBIENT_POSES.filter((p) => p !== exclude)
    : AMBIENT_POSES;
  const choices = pool.length > 0 ? pool : AMBIENT_POSES;
  const idx = Math.min(choices.length - 1, Math.floor(rng() * choices.length));
  return choices[idx];
}

export function pickVisemeCode(delta: string): string {
  const ch = delta
    .replace(/[^a-zA-Z]/g, "")
    .slice(-1)
    .toLowerCase();
  switch (ch) {
    case "a":
      return "aa";
    case "e":
      return "E";
    case "i":
    case "y":
      return "I";
    case "o":
      return "O";
    case "u":
    case "w":
      return "U";
    case "m":
    case "b":
    case "p":
      return "PP";
    case "f":
    case "v":
      return "FF";
    case "s":
    case "z":
      return "SS";
    case "n":
    case "l":
      return "nn";
    case "t":
    case "d":
      return "DD";
    case "k":
    case "g":
      return "kk";
    case "r":
      return "RR";
    default:
      return "E";
  }
}
