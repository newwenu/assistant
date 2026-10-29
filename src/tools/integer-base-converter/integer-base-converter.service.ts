export const CHARSET =
  "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ+/";

export function convertBase({
  value,
  fromBase,
  toBase,
}: {
  value: string;
  fromBase: number;
  toBase: number;
}): string {
  if (!value) return "";
  const fromRange = CHARSET.slice(0, fromBase).split("");
  const toRange = CHARSET.slice(0, toBase).split("");

  let decValue = value
    .split("")
    .reverse()
    .reduce((carry: bigint, digit: string, index: number) => {
      const digitLower = digit.toLowerCase();
      const idx = fromRange.indexOf(digitLower);
      if (idx === -1 && digit !== digitLower) {
        const idxUpper = fromRange.indexOf(digit);
        if (idxUpper === -1) {
          throw new Error(`无效数字 "${digit}" (基 ${fromBase})`);
        }
        return (carry += BigInt(idxUpper) * BigInt(fromBase) ** BigInt(index));
      }
      if (idx === -1) {
        throw new Error(`无效数字 "${digit}" (基 ${fromBase})`);
      }
      return (carry += BigInt(idx) * BigInt(fromBase) ** BigInt(index));
    }, 0n);

  if (decValue === 0n) return "0";

  let result = "";
  while (decValue > 0n) {
    result = toRange[Number(decValue % BigInt(toBase))] + result;
    decValue = (decValue - (decValue % BigInt(toBase))) / BigInt(toBase);
  }
  return result;
}

export const PRESET_BASES = [
  { label: "二进制 (2)", base: 2 },
  { label: "八进制 (8)", base: 8 },
  { label: "十进制 (10)", base: 10 },
  { label: "十六进制 (16)", base: 16 },
  { label: "Base64 (64)", base: 64 },
];
