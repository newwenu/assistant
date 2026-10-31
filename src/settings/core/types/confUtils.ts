export function toConfRecord(value: unknown): Record<string, unknown> | null {
  if (value == null) return null;
  return value as unknown as Record<string, unknown>;
}
