import type { AppConf, OrbConf, MascotConf, VaultConf } from "./confTypes";

type ConfUnion = AppConf | OrbConf | MascotConf | VaultConf;

export function toConfRecord(
  conf: ConfUnion | null,
): Record<string, unknown> | null {
  if (!conf) return null;
  return conf as unknown as Record<string, unknown>;
}
