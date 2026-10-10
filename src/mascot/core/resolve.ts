export function resolveVisemeCode(
  code: string,
  visemeCodes: readonly string[],
): string {
  const lower = code.toLowerCase();
  const match = visemeCodes.find((c) => c.toLowerCase() === lower);
  if (match) return match;
  const sil = visemeCodes.find((c) => c.toLowerCase() === "sil");
  return sil ?? visemeCodes[0];
}

export function pickIdleFlourish(
  cycle: string[],
  exclude?: string,
  rng: () => number = Math.random,
): string {
  const pool = cycle.filter((p) => p !== exclude);
  if (pool.length === 0) return cycle[0] ?? "";
  const idx = Math.min(pool.length - 1, Math.floor(rng() * pool.length));
  return pool[idx];
}

export interface VisemeShape {
  openness: number;
  width: number;
}

export function lerpViseme(
  a: VisemeShape,
  b: VisemeShape,
  t: number,
): VisemeShape {
  const k = Math.max(0, Math.min(1, t));
  return {
    openness: a.openness + (b.openness - a.openness) * k,
    width: a.width + (b.width - a.width) * k,
  };
}
