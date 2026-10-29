import JSON5 from "json5";

export function sortObjectKeys(o: unknown): unknown {
  if (Array.isArray(o)) return o.map(sortObjectKeys);
  if (o && typeof o === "object") {
    const sorted: Record<string, unknown> = {};
    const keys = Object.keys(o);
    keys.sort();
    for (const k of keys)
      sorted[k] = sortObjectKeys((o as Record<string, unknown>)[k]);
    return sorted;
  }
  return o;
}

export function formatJson(
  rawJson: string,
  indentSize: number,
  sortKeys: boolean,
): string | null {
  try {
    const obj: unknown = JSON5.parse(rawJson);
    const processed = sortKeys ? sortObjectKeys(obj) : obj;
    return JSON.stringify(processed, null, indentSize);
  } catch {
    return null;
  }
}

export function minifyJson(rawJson: string): string | null {
  try {
    return JSON.stringify(JSON5.parse(rawJson));
  } catch {
    return null;
  }
}
