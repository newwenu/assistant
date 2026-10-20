export interface GroupCapture {
  name: string;
  value: string;
  start: number;
  end: number;
}

export interface MatchResult {
  match: string;
  index: number;
  length: number;
  captures: GroupCapture[];
  namedGroups: GroupCapture[];
}

export const PATTERN_LIBRARY = [
  {
    name: "邮箱",
    pattern: "[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\\.[A-Za-z]{2,}",
    flags: "g",
  },
  { name: "手机号(CN)", pattern: "(?<!\\d)1[3-9]\\d{9}(?!\\d)", flags: "g" },
  {
    name: "URL",
    pattern: "https?://[\\w\\-._~:/?#\\[\\]@!$&'()*+,;=%]+",
    flags: "gi",
  },
  {
    name: "IPv4",
    pattern:
      "\\b(?:(?:25[0-5]|2[0-4]\\d|[01]?\\d?\\d)\\.){3}(?:25[0-5]|2[0-4]\\d|[01]?\\d?\\d)\\b",
    flags: "g",
  },
  {
    name: "UUID",
    pattern:
      "[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}",
    flags: "gi",
  },
  { name: "十六进制色", pattern: "#(?:[0-9a-fA-F]{3}){1,2}\\b", flags: "g" },
  {
    name: "日期 YYYY-MM-DD",
    pattern: "\\b(\\d{4})-(0[1-9]|1[0-2])-(0[1-9]|[12]\\d|3[01])\\b",
    flags: "g",
  },
  {
    name: "Semver",
    pattern:
      "\\b\\d+\\.\\d+\\.\\d+(?:-[0-9A-Za-z.-]+)?(?:\\+[0-9A-Za-z.-]+)?\\b",
    flags: "g",
  },
];

export function matchRegex(
  pattern: string,
  text: string,
  flags: string,
  isGlobal: boolean,
): { matches: MatchResult[]; error: string | null } {
  try {
    const regex = new RegExp(pattern, flags);
    const results: MatchResult[] = [];
    let m: RegExpExecArray | null;
    let lastIndex = -1;

    while ((m = regex.exec(text)) !== null) {
      if (isGlobal && regex.lastIndex === lastIndex) break;

      const captures: GroupCapture[] = [];
      const namedGroups: GroupCapture[] = [];

      const indices = (
        m as RegExpExecArray & { indices?: (readonly [number, number])[] }
      ).indices;

      for (let i = 1; i < m.length; i++) {
        const val = m[i];
        if (val !== undefined) {
          const idxRange = indices?.[i];
          captures.push({
            name: `$${i}`,
            value: val,
            start: idxRange ? idxRange[0] : -1,
            end: idxRange ? idxRange[1] : -1,
          });
        }
      }

      if (m.groups) {
        const namedIndices = (
          indices as unknown as {
            groups?: Record<string, readonly [number, number]>;
          }
        )?.groups;
        for (const [groupName, groupValue] of Object.entries(m.groups)) {
          if (groupValue !== undefined) {
            const idxRange = namedIndices?.[groupName];
            namedGroups.push({
              name: groupName,
              value: groupValue,
              start: idxRange ? idxRange[0] : -1,
              end: idxRange ? idxRange[1] : -1,
            });
          }
        }
      }

      results.push({
        match: m[0],
        index: m.index,
        length: m[0].length,
        captures,
        namedGroups,
      });

      lastIndex = regex.lastIndex;
      if (!isGlobal) break;
      if (m[0].length === 0) regex.lastIndex++;
    }

    return { matches: results, error: null };
  } catch (e: unknown) {
    return { matches: [], error: (e as Error).message };
  }
}

export function buildHighlightHtml(
  testText: string,
  matches: MatchResult[],
): string {
  if (matches.length === 0) return testText;

  const parts: { text: string; isMatch: boolean }[] = [];
  let lastIdx = 0;
  const sorted = [...matches].sort((a, b) => a.index - b.index);
  for (const r of sorted) {
    if (r.index > lastIdx) {
      parts.push({ text: testText.slice(lastIdx, r.index), isMatch: false });
    }
    parts.push({ text: r.match, isMatch: true });
    lastIdx = r.index + r.length;
  }
  if (lastIdx < testText.length) {
    parts.push({ text: testText.slice(lastIdx), isMatch: false });
  }

  const esc = (s: string) =>
    s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  return parts
    .map((p) =>
      p.isMatch
        ? `<mark style="background:var(--mark-bg);padding:0 2px;border-radius:2px">${esc(p.text)}</mark>`
        : esc(p.text),
    )
    .join("");
}

export function buildFlags(options: {
  global: boolean;
  ignoreCase: boolean;
  multiline: boolean;
  dotAll: boolean;
  unicode: boolean;
  unicodeSets: boolean;
}): string {
  let flags = "d";
  if (options.global) flags += "g";
  if (options.ignoreCase) flags += "i";
  if (options.multiline) flags += "m";
  if (options.dotAll) flags += "s";
  if (options.unicode) flags += "u";
  if (options.unicodeSets) flags += "v";
  return flags;
}
