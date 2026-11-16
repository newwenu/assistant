export const NO_VALUE = "—";

export function formatTime(millis: number): string {
  return new Date(millis).toLocaleString();
}

export function summarizeResource(resource: string): string {
  const parts = resource.split(/[\\/]+/).filter(Boolean);
  if (parts.length === 0) return resource;
  const base = parts[parts.length - 1];
  return parts.length === 1 ? base : `…/${base}`;
}
