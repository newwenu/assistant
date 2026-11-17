import type {
  Rive,
  ViewModel,
  ViewModelInstance,
} from "@rive-app/react-webgl2";
import type { RiveManifestDto } from "@/types/protocol.contract";

import { BUILTIN_FACES } from "../../core/types";

export interface RivePropertyDescriptor {
  name: string;
  type: string;
  enumName: string | null;
}

export interface RiveSchema {
  stateMachines: string[];
  properties: RivePropertyDescriptor[];
  enumValues: Record<string, string[]>;
}

export const EMPTY_RIVE_SCHEMA: RiveSchema = {
  stateMachines: [],
  properties: [],
  enumValues: {},
};

export interface RiveBindings {
  pose: string;
  viseme: string;
  primaryColor: string;
  secondaryColor: string;
}

export const FALLBACK_RIVE_BINDINGS: RiveBindings = {
  pose: "pose",
  viseme: "mouthVisemeCode",
  primaryColor: "primaryColor",
  secondaryColor: "secondaryColor",
};

const ENUM_TYPE = "enumType";
const COLOR_TYPE = "color";

const POSE_CANDIDATES = [
  "pose",
  "mood",
  "expression",
  "emotion",
  "state",
  "face",
];
const VISEME_CANDIDATES = [
  "mouthvisemecode",
  "viseme",
  "visemecode",
  "mouthshape",
  "mouth",
  "phoneme",
  "lip",
];
const PRIMARY_COLOR_CANDIDATES = [
  "primarycolor",
  "primary",
  "maincolor",
  "basecolor",
  "color1",
];
const SECONDARY_COLOR_CANDIDATES = [
  "secondarycolor",
  "secondary",
  "accentcolor",
  "color2",
];

function readList<T>(read: () => readonly T[] | null | undefined): T[] {
  try {
    const value = read();
    return value == null ? [] : Array.from(value);
  } catch {
    return [];
  }
}

function normalizeName(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9]/g, "");
}

export function readRiveSchema(
  rive: Rive | null,
  viewModel: ViewModel | null,
  vmInstance: ViewModelInstance | null,
): RiveSchema {
  const stateMachines = readList(() => rive?.stateMachineNames);

  const properties: RivePropertyDescriptor[] = [];
  for (const raw of readList(() => viewModel?.properties)) {
    if (!raw?.name) continue;
    properties.push({
      name: raw.name,
      type: String(raw.type),
      enumName: raw.enumName ?? null,
    });
  }

  const enumValues: Record<string, string[]> = {};
  if (vmInstance) {
    for (const property of properties) {
      if (property.type !== ENUM_TYPE) continue;
      const values = readList(() => vmInstance.enum(property.name)?.values);
      if (values.length > 0) enumValues[property.name] = values;
    }
  }

  return { stateMachines, properties, enumValues };
}

function matchProperty(
  schema: RiveSchema,
  name: string | null | undefined,
): string | null {
  if (!name) return null;
  const exact = schema.properties.find((p) => p.name === name);
  if (exact) return exact.name;
  const lower = name.toLowerCase();
  const insensitive = schema.properties.find(
    (p) => p.name.toLowerCase() === lower,
  );
  if (insensitive) return insensitive.name;
  const normalized = normalizeName(name);
  const loose = schema.properties.find(
    (p) => normalizeName(p.name) === normalized,
  );
  return loose ? loose.name : null;
}

function pickEnumProperty(
  schema: RiveSchema,
  candidates: readonly string[],
  probes: readonly string[],
): string | null {
  const enums = schema.properties.filter((p) => p.type === ENUM_TYPE);
  for (const candidate of candidates) {
    const hit = enums.find((p) => normalizeName(p.name) === candidate);
    if (hit) return hit.name;
  }
  for (const property of enums) {
    const values = (schema.enumValues[property.name] ?? []).map((v) =>
      v.toLowerCase(),
    );
    if (probes.some((probe) => values.includes(probe))) return property.name;
  }
  return null;
}

function pickColorProperty(
  schema: RiveSchema,
  candidates: readonly string[],
  fallbackIndex: number,
): string | null {
  const colors = schema.properties.filter((p) => p.type === COLOR_TYPE);
  for (const candidate of candidates) {
    const hit = colors.find((p) => normalizeName(p.name) === candidate);
    if (hit) return hit.name;
  }
  return colors[fallbackIndex]?.name ?? null;
}

export function resolveRiveBindings(
  schema: RiveSchema,
  manifest?: RiveManifestDto | null,
): RiveBindings {
  const declared = manifest?.properties ?? null;
  return {
    pose:
      matchProperty(schema, declared?.pose) ??
      declared?.pose ??
      pickEnumProperty(schema, POSE_CANDIDATES, ["idle"]) ??
      FALLBACK_RIVE_BINDINGS.pose,
    viseme:
      matchProperty(schema, declared?.viseme) ??
      declared?.viseme ??
      pickEnumProperty(schema, VISEME_CANDIDATES, ["sil", "pp"]) ??
      FALLBACK_RIVE_BINDINGS.viseme,
    primaryColor:
      matchProperty(schema, declared?.primaryColor) ??
      declared?.primaryColor ??
      pickColorProperty(schema, PRIMARY_COLOR_CANDIDATES, 0) ??
      FALLBACK_RIVE_BINDINGS.primaryColor,
    secondaryColor:
      matchProperty(schema, declared?.secondaryColor) ??
      declared?.secondaryColor ??
      pickColorProperty(schema, SECONDARY_COLOR_CANDIDATES, 1) ??
      FALLBACK_RIVE_BINDINGS.secondaryColor,
  };
}

function findPose(
  poses: readonly { raw: string; normalized: string }[],
  name: string,
): string | null {
  const normalized = normalizeName(name);
  if (normalized.length === 0) return null;
  return poses.find((p) => p.normalized === normalized)?.raw ?? null;
}

export function deriveFaceToPose(
  poses: readonly string[],
  baseStates: Record<string, string>,
): Record<string, string> {
  if (poses.length === 0) return { ...baseStates };
  const known = poses.map((raw) => ({ raw, normalized: normalizeName(raw) }));
  const idlePose = findPose(known, "idle") ?? poses[0];
  const derived: Record<string, string> = {};
  for (const face of BUILTIN_FACES) {
    const mapped = findPose(known, baseStates[face] ?? "");
    if (mapped) {
      derived[face] = mapped;
      continue;
    }
    const direct = findPose(known, face);
    if (direct) {
      derived[face] = direct;
      continue;
    }
    const normalizedFace = normalizeName(face);
    const loose = known.find(
      (p) =>
        p.normalized.length >= 4 &&
        (p.normalized.includes(normalizedFace) ||
          normalizedFace.includes(p.normalized)),
    );
    derived[face] = loose?.raw ?? idlePose;
  }
  return derived;
}

export interface FilteredFaceToPose {
  kept: Record<string, string>;
  dropped: string[];
}

/** 丢弃目标 pose 不在枚举全集中的条目，并把保留项归一为素材里的实际写法。 */
export function filterPoseMap(
  map: Record<string, string>,
  poses: readonly string[],
): FilteredFaceToPose {
  const known = new Map(poses.map((p) => [p.toLowerCase(), p]));
  const kept: Record<string, string> = {};
  const dropped: string[] = [];
  for (const [key, pose] of Object.entries(map)) {
    const hit = known.get(pose.toLowerCase());
    if (hit) {
      kept[key] = hit;
    } else {
      dropped.push(`${key} -> ${pose}`);
    }
  }
  return { kept, dropped };
}

export const filterFaceToPose = filterPoseMap;

/** 场景映射与表情映射共享同一套校验：目标必须是素材真实存在的 pose。 */
export const filterSceneToPose = filterPoseMap;
