const RANGE_KEYS = ["all", "24h", "7d", "30d"] as const;

export type RangeKey = (typeof RANGE_KEYS)[number];

export { RANGE_KEYS };

export const DEFAULT_LIMIT = 50;
export const LIMIT_OPTIONS = [50, 100, 300, 500];

const RANGE_HOURS: Record<RangeKey, number | null> = {
  all: null,
  "24h": 24,
  "7d": 168,
  "30d": 720,
};

export function rangeToSince(range: RangeKey): number | null {
  const hours = RANGE_HOURS[range];
  return hours === null ? null : Date.now() - hours * 3_600_000;
}
