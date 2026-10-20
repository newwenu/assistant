import { v1, v3, v4, v5, v7, NIL } from "uuid";

export const NAMESPACES: Record<string, string> = {
  DNS: "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
  URL: "6ba7b811-9dad-11d1-80b4-00c04fd430c8",
  OID: "6ba7b812-9dad-11d1-80b4-00c04fd430c8",
  X500: "6ba7b814-9dad-11d1-80b4-00c04fd430c8",
};

export const NAMESPACE_OPTIONS = Object.keys(NAMESPACES).map((k) => ({
  value: k,
  label: `${k} (${NAMESPACES[k].slice(0, 8)}...)`,
}));

export const VERSION_OPTIONS = [
  { value: "NIL", label: "NIL (全零)" },
  { value: "v1", label: "v1 (时间)" },
  { value: "v3", label: "v3 (MD5 命名)" },
  { value: "v4", label: "v4 (随机)" },
  { value: "v5", label: "v5 (SHA1 命名)" },
  { value: "v7", label: "v7 (时间排序)" },
];

export function generateUuids(
  version: string,
  count: number,
  namespace: string,
  name: string,
): string[] {
  const results: string[] = [];
  for (let i = 0; i < count; i++) {
    switch (version) {
      case "NIL":
        results.push(NIL);
        break;
      case "v1":
        results.push(v1());
        break;
      case "v3":
        results.push(v3(name, NAMESPACES[namespace] ?? NAMESPACES.URL));
        break;
      case "v4":
        results.push(v4());
        break;
      case "v5":
        results.push(v5(name, NAMESPACES[namespace] ?? NAMESPACES.URL));
        break;
      case "v7":
        results.push(v7());
        break;
      default:
        results.push(v4());
    }
  }
  return results;
}
