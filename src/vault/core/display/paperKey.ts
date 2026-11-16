export const PAPER_CHARS = 40;
export const PAPER_GROUP_LEN = 8;
export const PAPER_GROUPS = PAPER_CHARS / PAPER_GROUP_LEN;

const CROCKFORD_ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";

export function normalizePaperInput(raw: string): string {
  let out = "";
  for (const ch of raw) {
    let mapped = ch;
    if (ch >= "\uFF10" && ch <= "\uFF19") {
      mapped = String.fromCharCode(ch.charCodeAt(0) - 0xff10 + 0x30);
    } else if (ch >= "\uFF21" && ch <= "\uFF3A") {
      mapped = String.fromCharCode(ch.charCodeAt(0) - 0xff21 + 0x41);
    }
    const upper = mapped.toUpperCase();
    if (/[- \t\r\n\u3000]/.test(upper)) continue;
    let final = upper;
    if (upper === "O") final = "0";
    else if (upper === "I" || upper === "L") final = "1";
    else if (upper === "U") final = "V";
    out += final;
  }
  return out;
}

export function isPaperChar(ch: string): boolean {
  return CROCKFORD_ALPHABET.includes(ch);
}

export function groupPaperKey(normalized: string): string[] {
  const groups: string[] = [];
  for (let i = 0; i < normalized.length; i += PAPER_GROUP_LEN) {
    groups.push(normalized.slice(i, i + PAPER_GROUP_LEN));
  }
  return groups;
}

export function formatPaperKey(normalized: string): string {
  return groupPaperKey(normalized).join("-");
}

export function paperInputError(
  normalized: string,
): "short" | "long" | "char" | null {
  if (normalized.length < PAPER_CHARS) return "short";
  if (normalized.length > PAPER_CHARS) return "long";
  for (const ch of normalized) {
    if (!isPaperChar(ch)) return "char";
  }
  return null;
}
