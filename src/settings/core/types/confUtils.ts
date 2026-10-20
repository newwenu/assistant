import type { AppConf, OrbConf, MascotConf } from "./confTypes";

type ConfUnion = AppConf | OrbConf | MascotConf;

export function toConfRecord(
  conf: ConfUnion | null,
): Record<string, unknown> | null {
  if (!conf) return null;
  return conf as unknown as Record<string, unknown>;
}
