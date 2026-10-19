export const LOWERCASE = "abcdefghijklmnopqrstuvwxyz";
export const UPPERCASE = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
export const DIGITS = "0123456789";
export const SPECIAL = "!@#$%^&*()_+-=[]{}|;:',.<>?/`~";

export const CHARSET_SIZES = {
  lowercase: LOWERCASE.length,
  uppercase: UPPERCASE.length,
  digits: DIGITS.length,
  special: SPECIAL.length,
} as const;

function buildSpecialCharRegex(chars: string): RegExp {
  const escaped = chars
    .replace(/[.*+?^${}()|[\]\\]/g, "\\$&")
    .replace(/-/g, "\\-");
  return new RegExp(`[${escaped}]`);
}

const SPECIAL_REGEX = buildSpecialCharRegex(SPECIAL);

export function getCharsetLength(password: string): number {
  let len = 0;
  if (/[a-z]/.test(password)) len += CHARSET_SIZES.lowercase;
  if (/[A-Z]/.test(password)) len += CHARSET_SIZES.uppercase;
  if (/\d/.test(password)) len += CHARSET_SIZES.digits;
  if (SPECIAL_REGEX.test(password)) len += CHARSET_SIZES.special;
  return len;
}

export interface CrackTimeEstimation {
  entropy: number;
  charsetLength: number;
  passwordLength: number;
  secondsToCrack: number;
  score: number;
}

export function getCrackTimeEstimation(
  password: string,
  guessesPerSecond = 1e9,
): CrackTimeEstimation {
  const charsetLength = getCharsetLength(password);
  const passwordLength = password.length;
  if (charsetLength === 0 || passwordLength === 0) {
    return {
      entropy: 0,
      charsetLength: 0,
      passwordLength,
      secondsToCrack: 0,
      score: 0,
    };
  }
  const entropy = Math.log2(charsetLength) * passwordLength;
  const secondsToCrack = 2 ** entropy / guessesPerSecond;
  const score = Math.min(entropy / 128, 1);
  return { entropy, charsetLength, passwordLength, secondsToCrack, score };
}

export function formatDuration(seconds: number): string {
  if (seconds <= 0.001) return "瞬间";
  if (seconds <= 1) return "不到 1 秒";

  const units = [
    { unit: "千年", s: 31536000000 },
    { unit: "世纪", s: 3153600000 },
    { unit: "十年", s: 315360000 },
    { unit: "年", s: 31536000 },
    { unit: "月", s: 2592000 },
    { unit: "周", s: 604800 },
    { unit: "天", s: 86400 },
    { unit: "小时", s: 3600 },
    { unit: "分钟", s: 60 },
    { unit: "秒", s: 1 },
  ];

  const parts: string[] = [];
  let remaining = seconds;
  for (const { unit, s } of units) {
    const qty = Math.floor(remaining / s);
    if (qty > 0) {
      parts.push(`${qty} ${unit}`);
      remaining %= s;
    }
    if (parts.length >= 2) break;
  }
  return parts.join(" ") || "不到 1 秒";
}

export interface StrengthLevel {
  label: string;
  color: string;
  percent: number;
}

export function getStrengthLevel(score: number): StrengthLevel {
  if (score < 0.25)
    return { label: "极弱", color: "#ff4d4f", percent: score * 100 };
  if (score < 0.5)
    return { label: "弱", color: "#faad14", percent: score * 100 };
  if (score < 0.75)
    return { label: "中等", color: "#1677ff", percent: score * 100 };
  return { label: "强", color: "#52c41a", percent: score * 100 };
}

function secureRandomIndex(charsetLen: number): number {
  const limit = Math.floor(0x100000000 / charsetLen) * charsetLen;
  const buf = new Uint32Array(1);
  let v: number;
  do {
    crypto.getRandomValues(buf);
    v = buf[0];
  } while (v >= limit);
  return v % charsetLen;
}

export function generatePassword(
  length: number,
  useLower: boolean,
  useUpper: boolean,
  useDigits: boolean,
  useSpecial: boolean,
): string {
  let charset = "";
  if (useLower) charset += LOWERCASE;
  if (useUpper) charset += UPPERCASE;
  if (useDigits) charset += DIGITS;
  if (useSpecial) charset += SPECIAL;
  if (!charset) charset = LOWERCASE;

  return Array.from(
    { length },
    () => charset[secureRandomIndex(charset.length)],
  ).join("");
}
